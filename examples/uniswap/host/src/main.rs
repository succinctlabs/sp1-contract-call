use alloy_primitives::{address, Address};
use alloy_provider::ReqwestProvider;
use alloy_rpc_types::BlockNumberOrTag;
use alloy_sol_macro::sol;
use alloy_sol_types::SolValue;
use sp1_cc_client_executor::ContractInput;
use sp1_cc_host_executor::HostExecutor;
use sp1_sdk::{utils, ProverClient, SP1Stdin};
use url::Url;

sol! {
    /// Simplified interface of the IUniswapV3PoolState interface.
    interface IUniswapV3PoolState {
        function slot0() external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked);
    }
}

sol! {
    struct UniswapOutput {
        address contractAddress;
        address callerAddress;
        bytes contractCallData;
        uint160 sqrtPriceX96;
        bytes32 blockHash;
    }
}

/// Address of Uniswap V3 pool.
const CONTRACT: Address = address!("1d42064Fc4Beb5F8aAF85F4617AE8b3b5B8Bd801");

/// Address of the caller.
const CALLER: Address = address!("0000000000000000000000000000000000000000");

/// The ELF we want to execute inside the zkVM.
const ELF: &[u8] = include_bytes!("../../client/elf/riscv32im-succinct-zkvm-elf");

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Setup logging.
    utils::setup_logger();

    // Which block transactions are executed on.
    let block_number = BlockNumberOrTag::Latest;

    // Prepare the host executor.
    //
    // Use `RPC_URL` to get all of the necessary state for the smart contract call.
    let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| panic!("Missing RPC_URL"));
    let provider = ReqwestProvider::new_http(Url::parse(&rpc_url)?);
    let mut host_executor = HostExecutor::new(provider.clone(), block_number).await?;

    // Keep track of the block hash. Later, validate the client's execution against this.
    let block_hash = host_executor.header.hash_slow();

    // Make the call to the slot0 function.
    let slot0_call = IUniswapV3PoolState::slot0Call {};
    let _price_x96 = host_executor
        .execute(ContractInput {
            contract_address: CONTRACT,
            caller_address: CALLER,
            calldata: slot0_call,
        })
        .await?
        .sqrtPriceX96;

    // Now that we've executed all of the calls, get the `EVMStateSketch` from the host executor.
    let input = host_executor.finalize().await?;

    // Feed the sketch into the client.
    let input_bytes = bincode::serialize(&input)?;
    let mut stdin = SP1Stdin::new();
    stdin.write(&input_bytes);

    // Create a `ProverClient`.
    let client = ProverClient::new();

    // Execute the program using the `ProverClient.execute` method, without generating a proof.
    let (_, report) = client.execute(ELF, stdin.clone()).run().unwrap();
    println!("executed program with {} cycles", report.total_instruction_count());

    // Generate the proof for the given program and input.
    let (pk, vk) = client.setup(ELF);
    let proof = client.prove(&pk, stdin).run().unwrap();
    println!("generated proof");

    // Read the public values, and deserialize them.
    let public_vals = UniswapOutput::abi_decode(proof.public_values.as_slice(), true)?;

    // Check that the provided block hash matches the one in the proof.
    assert_eq!(public_vals.blockHash, block_hash);

    // Read the output, and then calculate the uniswap exchange rate.
    //
    // Note that this output is read from values commited to in the program using
    // `sp1_zkvm::io::commit`.
    let sqrt_price = f64::from(public_vals.sqrtPriceX96) / 2f64.powi(96);
    let price = sqrt_price * sqrt_price;
    println!("Proven exchange rate is: {}%", price);

    // Verify proof and public values.
    client.verify(&proof, &vk).expect("verification failed");
    println!("successfully generated and verified proof for the program!");
    Ok(())
}
