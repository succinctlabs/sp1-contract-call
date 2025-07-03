use alloy::network::AnyNetwork;
use alloy_primitives::{address, Address};
use alloy_provider::RootProvider;
use alloy_rpc_types::BlockNumberOrTag;
use alloy_sol_macro::sol;
use alloy_sol_types::SolValue;
use clap::Parser;
use serde::{Deserialize, Serialize};
use sp1_cc_client_executor::ContractPublicValues;
use sp1_cc_host_executor::{EvmSketch, Genesis};
use sp1_sdk::{include_elf, utils, ProverClient, SP1Stdin};
use url::Url;

/// Address of a Uniswap V3 pool.
const POOL_CONTRACT: Address = address!("3289680dD4d6C10bb19b899729cda5eEF58AEfF1");
/// Address of the Uniswap verifier contract on Sepolia.
const UNISWAP_CALL_CONTRACT: Address = address!("2637E77e371e8b001ac0CB8A690B9991cf0601f0");

/// The ELF we want to execute inside the zkVM.
const ELF: &[u8] = include_elf!("uniswap-client");

sol!(
    #[sol(rpc)]
    "../contracts/src/UniswapCall.sol"
);

sol! {
    /// Simplified interface of the IUniswapV3PoolState interface.
    interface IUniswapV3PoolState {
        function slot0() external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked);
    }
}

/// A fixture that can be used to test the verification of SP1 zkVM proofs inside Solidity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SP1CCProofFixture {
    vkey: String,
    public_values: String,
    proof: String,
}

/// The arguments for the command.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long)]
    execution_block: u64,

    #[clap(long)]
    reference_block: Option<u64>,

    #[clap(long, env)]
    eth_sepolia_rpc_url: Url,

    #[clap(long, env)]
    beacon_sepolia_rpc_url: Option<Url>,

    #[clap(long)]
    active_fork_name: String,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenv::dotenv().ok();

    // Setup logging.
    utils::setup_logger();

    // Parse the command line arguments.
    let args = Args::parse();

    // Which block transactions are executed on.
    let block_number = BlockNumberOrTag::Number(args.execution_block);

    let provider = RootProvider::<AnyNetwork>::new_http(args.eth_sepolia_rpc_url.clone());

    // Create a `ProverClient`.
    let client = ProverClient::from_env();

    let (pk, _) = client.setup(ELF);

    let contract = UniswapCall::new(UNISWAP_CALL_CONTRACT, provider.clone());

    // Prepare the sketch.
    let sketch_builder = EvmSketch::builder()
        .with_genesis(Genesis::Sepolia)
        .at_block(block_number)
        .el_rpc_url(args.eth_sepolia_rpc_url.clone());

    let sketch = if let Some(beacon_sepolia_rpc_url) = args.beacon_sepolia_rpc_url {
        let sketch_builder = sketch_builder.cl_rpc_url(beacon_sepolia_rpc_url.clone());

        if let Some(reference_block) = args.reference_block {
            let sketch_builder = sketch_builder.at_reference_block(reference_block);

            sketch_builder.build().await?
        } else {
            sketch_builder.build().await?
        }
    } else {
        sketch_builder.build().await?
    };

    // Keep track of the block root. Later, validate the client's execution against this.
    let block_root = sketch.anchor.resolve().hash;

    // Make the call to the slot0 function.
    let slot0_call = IUniswapV3PoolState::slot0Call {};
    let _price_x96_bytes = sketch.call(POOL_CONTRACT, Address::default(), slot0_call).await?;

    // Now that we've executed all of the calls, get the `EvmSketchInput` from the sketch.
    let input = sketch.finalize().await?;

    // Feed the sketch into the client.
    let input_bytes = bincode::serialize(&input)?;
    let mut stdin = SP1Stdin::new();
    stdin.write(&input_bytes);

    // Execute the program using the `ProverClient.execute` method, without generating a proof.
    let (_, report) = client.execute(ELF, &stdin).run().unwrap();
    println!("executed program with {} cycles", report.total_instruction_count());

    // Generate the proof for the given program and input.
    let proof = client.prove(&pk, &stdin).groth16().run().unwrap();
    println!("generated proof");

    // Read the public values, and deserialize them.
    let public_vals = ContractPublicValues::abi_decode(proof.public_values.as_slice())?;

    // Check that the provided block root matches the one in the proof.
    assert_eq!(public_vals.anchorHash, block_root);
    println!("verified block root");

    // Verify onchain.
    let sqrt_price_x96 = contract
        .verifyUniswapCallProof(
            proof.public_values.to_vec().into(),
            proof.bytes().into(),
            args.active_fork_name,
        )
        .call()
        .await
        .unwrap();
    println!("verified proof onchain");

    let sqrt_price = f64::from(sqrt_price_x96) / 2f64.powi(96);
    let price = sqrt_price * sqrt_price;
    println!("Proven exchange rate is: {}%", price);

    Ok(())
}
