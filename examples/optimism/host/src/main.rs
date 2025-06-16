use alloy::sol;
use alloy_primitives::{address, Address};
use alloy_sol_types::{SolCall, SolType};
use sp1_cc_client_executor::ContractPublicValues;
use sp1_cc_host_executor::EvmSketch;
use sp1_sdk::{include_elf, utils, ProverClient, SP1Stdin};
use url::Url;

const CONTRACT: Address = address!("0x4200000000000000000000000000000000000015");

sol! {
    interface IL1Block {
        function basefee() external view returns (uint256);
    }
}

/// The ELF we want to execute inside the zkVM.
const ELF: &[u8] = include_elf!("optimism-client");

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenv::dotenv().ok();

    // Setup logging.
    utils::setup_logger();

    // Prepare the host executor.
    //
    // Use `OPTIMISM_RPC_URL` to get all of the necessary state for the smart contract call.
    let rpc_url = std::env::var("OPTIMISM_RPC_URL")
        .unwrap_or_else(|_| panic!("Missing OPTIMISM_RPC_URL in env"));
    let sketch =
        EvmSketch::builder().optimism_mainnet().el_rpc_url(Url::parse(&rpc_url)?).build().await?;

    sketch.call(CONTRACT, Address::default(), IL1Block::basefeeCall).await?;

    // Now that we've executed all of the calls, get the `EvmSketchInput` from the host executor.
    let input = sketch.finalize().await?;

    // Feed the sketch into the client.
    let input_bytes = bincode::serialize(&input)?;
    let mut stdin = SP1Stdin::new();
    stdin.write(&input_bytes);

    let client = ProverClient::from_env();

    // Execute the program using the `ProverClient.execute` method, without generating a proof.
    let (_, report) = client.execute(ELF, &stdin).run().unwrap();
    println!("executed program with {} cycles", report.total_instruction_count());

    // Generate the proof for the given program and input.
    let (pk, _) = client.setup(ELF);
    let proof = client.prove(&pk, &stdin).run().unwrap();
    println!("generated proof");

    // Read the public values, and deserialize them.
    let public_vals = ContractPublicValues::abi_decode(proof.public_values.as_slice())?;

    // Print the fetched rates.
    let base_fee = IL1Block::basefeeCall::abi_decode_returns(&public_vals.contractOutput)?;
    println!("Base fee: \n{:?}", base_fee);

    Ok(())
}
