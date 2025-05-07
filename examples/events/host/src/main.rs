use alloy::{
    primitives::{address, Address},
    providers::RootProvider,
    rpc::types::Filter,
};
use alloy_sol_types::SolEvent;
use clap::Parser;
use eyre::bail;
use sp1_cc_host_executor::EventLogsPrefetcher;
use sp1_sdk::{include_elf, utils, ProverClient, SP1Stdin};
use swap_events_client::IERC20;
use url::Url;

/// The ELF we want to execute inside the zkVM.
const DECODED_ELF: &[u8] = include_elf!("decoded");
const METADATA_ELF: &[u8] = include_elf!("metadata");
const WETH: Address = address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");

/// The arguments for the command.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long)]
    decoded: bool,

    #[clap(long)]
    metadata: bool,

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

    if args.decoded == args.metadata {}

    let rpc_url =
        std::env::var("ETH_RPC_URL").unwrap_or_else(|_| panic!("Missing ETH_RPC_URL in env"));
    let provider = RootProvider::new_http(Url::parse(&rpc_url)?);

    // Create a `ProverClient`.
    let client = ProverClient::from_env();
    let mut stdin = SP1Stdin::new();
    let event_prefetcher = EventLogsPrefetcher::new(provider.clone());
    let filter = Filter::new()
        .address(WETH)
        .event(IERC20::Transfer::SIGNATURE)
        .from_block(22417000)
        .to_block(22417100);

    let elf = match (args.decoded, args.metadata) {
        (true, true) => bail!("only one of --decoded or --metadata should be set"),
        (true, false) => {
            let events_input =
                event_prefetcher.prefetch_events::<IERC20::Transfer>(&filter).await?;
            stdin.write(&events_input);

            DECODED_ELF
        }
        (false, true) => {
            let logs_input = event_prefetcher.prefetch_logs(&filter).await?;
            stdin.write(&logs_input);

            METADATA_ELF
        }
        (false, false) => bail!("either --decoded or --metadata must be set"),
    };

    // Execute the program using the `ProverClient.execute` method, without generating a proof.
    let (_, report) = client.execute(elf, &stdin).run().unwrap();
    println!("executed program with {} cycles", report.total_instruction_count());

    // If the prove flag is not set, we return here.
    if !args.prove {
        return Ok(());
    }

    // Generate the proof for the given program and input.
    let (pk, vk) = client.setup(DECODED_ELF);
    let proof = client.prove(&pk, &stdin).plonk().run().unwrap();
    println!("generated proof");

    // Verify proof and public values.
    client.verify(&proof, &vk).expect("verification failed");
    println!("successfully generated and verified proof for the program!");
    Ok(())
}
