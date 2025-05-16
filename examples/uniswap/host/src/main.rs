use std::path::PathBuf;

use alloy::hex;
use alloy_primitives::{address, Address};
use alloy_rpc_types::BlockNumberOrTag;
use alloy_sol_macro::sol;
use alloy_sol_types::{SolCall, SolValue};
use clap::Parser;
use serde::{Deserialize, Serialize};
use sp1_cc_client_executor::{ContractInput, ContractPublicValues};
use sp1_cc_host_executor::EvmSketch;
use sp1_sdk::{include_elf, utils, HashableKey, ProverClient, SP1ProofWithPublicValues, SP1Stdin};
use url::Url;
use IUniswapV3PoolState::slot0Call;

sol! {
    /// Simplified interface of the IUniswapV3PoolState interface.
    interface IUniswapV3PoolState {
        function slot0() external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked);
    }
}

/// Address of Uniswap V3 pool.
const CONTRACT: Address = address!("1d42064Fc4Beb5F8aAF85F4617AE8b3b5B8Bd801");

/// The ELF we want to execute inside the zkVM.
const ELF: &[u8] = include_elf!("uniswap-client");

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
    prove: bool,

    #[clap(long, env)]
    eth_rpc_url: Url,
}

/// Generate a `SP1CCProofFixture`, and save it as a json file.
///
/// This is useful for verifying the proof of contract call execution on chain.
fn save_fixture(vkey: String, proof: &SP1ProofWithPublicValues) {
    let fixture = SP1CCProofFixture {
        vkey,
        public_values: format!("0x{}", hex::encode(proof.public_values.as_slice())),
        proof: format!("0x{}", hex::encode(proof.bytes())),
    };

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../contracts/src/fixtures");
    std::fs::create_dir_all(&fixture_path).expect("failed to create fixture path");
    std::fs::write(
        fixture_path.join("plonk-fixture.json".to_lowercase()),
        serde_json::to_string_pretty(&fixture).unwrap(),
    )
    .expect("failed to write fixture");
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenv::dotenv().ok();

    // Setup logging.
    utils::setup_logger();

    // Parse the command line arguments.
    let args = Args::parse();

    // Which block transactions are executed on.
    let block_number = BlockNumberOrTag::Number(20600000);

    // Prepare the host executor.
    let sketch =
        EvmSketch::builder().at_block(block_number).el_rpc_url(args.eth_rpc_url).build().await?;

    // Keep track of the block hash. Later, validate the client's execution against this.
    let block_hash = sketch.anchor.resolve().hash;

    // Make the call to the slot0 function.
    let slot0_call = IUniswapV3PoolState::slot0Call {};
    let _price_x96_bytes =
        sketch.call(ContractInput::new_call(CONTRACT, Address::default(), slot0_call)).await?;

    // Now that we've executed all of the calls, get the `EVMStateSketch` from the host executor.
    let input = sketch.finalize().await?;

    // Feed the sketch into the client.
    let input_bytes = bincode::serialize(&input)?;
    let mut stdin = SP1Stdin::new();
    stdin.write(&input_bytes);

    // Create a `ProverClient`.
    let client = ProverClient::from_env();

    // Execute the program using the `ProverClient.execute` method, without generating a proof.
    let (_, report) = client.execute(ELF, &stdin).run().unwrap();
    println!("executed program with {} cycles", report.total_instruction_count());

    // If the prove flag is not set, we return here.
    if !args.prove {
        return Ok(());
    }

    // Generate the proof for the given program and input.
    let (pk, vk) = client.setup(ELF);
    let proof = client.prove(&pk, &stdin).plonk().run().unwrap();
    println!("generated proof");

    // Read the public values, and deserialize them.
    let public_vals = ContractPublicValues::abi_decode(proof.public_values.as_slice())?;

    // Check that the provided block hash matches the one in the proof.
    assert_eq!(public_vals.anchorHash, block_hash);
    println!("verified block hash");

    // Read the output, and then calculate the uniswap exchange rate.
    //
    // Note that this output is read from values commited to in the program using
    // `sp1_zkvm::io::commit`.
    let sqrt_price_x96 = slot0Call::abi_decode_returns(&public_vals.contractOutput)?.sqrtPriceX96;
    let sqrt_price = f64::from(sqrt_price_x96) / 2f64.powi(96);
    let price = sqrt_price * sqrt_price;
    println!("Proven exchange rate is: {}%", price);

    // Save the proof, public values, and vkey to a json file.
    save_fixture(vk.bytes32(), &proof);
    println!("saved proof to plonk-fixture.json");

    // Verify proof and public values.
    client.verify(&proof, &vk).expect("verification failed");
    println!("successfully generated and verified proof for the program!");
    Ok(())
}
