use alloy::{providers::RootProvider, rpc::types::Filter};
use alloy_sol_types::SolEvent;
use clap::Parser;
use events_client::{IERC20, WETH};
use sp1_cc_host_executor::HostExecutor;
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

    let rpc_url =
        std::env::var("ETH_RPC_URL").unwrap_or_else(|_| panic!("Missing ETH_RPC_URL in env"));
    let provider = RootProvider::new_http(Url::parse(&rpc_url)?);

    let mut host_executor = HostExecutor::new(provider, 22417000.into()).await?;

    // Create a `ProverClient`.
    let client = ProverClient::from_env();
    let mut stdin = SP1Stdin::new();

    let filter = Filter::new()
        .address(WETH)
        .at_block_hash(host_executor.header.hash_slow())
        .event(IERC20::Transfer::SIGNATURE);

    host_executor.prefetch_logs(&filter).await.unwrap();

    let input = host_executor.finalize().await?;

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
