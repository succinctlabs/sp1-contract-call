use alloy::{eips::BlockNumberOrTag, rpc::types::Filter};
use alloy_sol_types::SolEvent;
use clap::Parser;
use events_client::{IERC20, WETH};
use sp1_cc_host_executor::EvmSketch;
use sp1_sdk::{include_elf, utils, ProverClient, SP1Stdin};
use url::Url;

/// The ELF we want to execute inside the zkVM.
const ELF: &[u8] = include_elf!("events-client");

/// The arguments for the command.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long)]
    prove: bool,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenv::dotenv().ok();

    // Setup logging.
    utils::setup_logger();

    // Parse the command line arguments.
    let args = Args::parse();

    let block_number = BlockNumberOrTag::Number(22417000);

    let rpc_url =
        std::env::var("ETH_RPC_URL").unwrap_or_else(|_| panic!("Missing ETH_RPC_URL in env"));
    let mut sketch = EvmSketch::builder()
        .at_block(block_number) // Get a recent blob to get the hash from.
        .el_rpc_url(Url::parse(&rpc_url)?)
        .build()
        .await?;

    // Create a `ProverClient`.
    let client = ProverClient::from_env();
    let mut stdin = SP1Stdin::new();

    let filter = Filter::new()
        .address(WETH)
        .at_block_hash(sketch.anchor.hash())
        .event(IERC20::Transfer::SIGNATURE);

    let _ = sketch.get_logs(&filter).await.unwrap();

    let input = sketch.finalize().await?;

    stdin.write(&input);

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

    // Verify proof and public values.
    client.verify(&proof, &vk).expect("verification failed");
    println!("successfully generated and verified proof for the program!");
    Ok(())
}
