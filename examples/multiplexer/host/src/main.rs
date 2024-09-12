use alloy_primitives::{address, Address, B256, U256};
use alloy_provider::ReqwestProvider;
use alloy_rpc_types::BlockNumberOrTag;
use alloy_sol_macro::sol;
use sp1_cc_client_executor::ContractInput;
use sp1_cc_host_executor::HostExecutor;
use sp1_sdk::{utils, ProverClient, SP1Stdin};
use url::Url;

sol! {
    /// Interface to the multiplexer contract. It gets the exchange rates of many tokens, including
    /// apxEth, ankrEth, and pufEth.
    interface IOracleHelper {
        function getRates(address[] memory collaterals) external view returns (uint256[] memory);
    }
}

/// Address of the multiplexer contract on Ethereum Mainnet.
const CONTRACT: Address = address!("0A8c00EcFA0816F4f09289ac52Fcb88eA5337526");

/// Address of the caller.
const CALLER: Address = address!("0000000000000000000000000000000000000000");

/// Inputs to the contract call.
const COLLATERALS: [Address; 12] = [
    address!("E95A203B1a91a908F9B9CE46459d101078c2c3cb"),
    address!("9Ba021B0a9b958B5E75cE9f6dff97C7eE52cb3E6"),
    address!("Be9895146f7AF43049ca1c1AE358B0541Ea49704"),
    address!("7f39C581F595B53c5cb19bD0b3f8dA6c935E2Ca0"),
    address!("A35b1B31Ce002FBF2058D22F30f95D405200A15b"),
    address!("D9A442856C234a39a81a089C06451EBAa4306a72"),
    address!("ae78736Cd615f374D3085123A210448E74Fc6393"),
    address!("A1290d69c65A6Fe4DF752f95823fae25cB99e5A7"),
    address!("ac3E018457B222d93114458476f3E3416Abbe38F"),
    address!("9D39A5DE30e57443BfF2A8307A4256c8797A3497"),
    address!("f951E335afb289353dc249e82926178EaC7DEd78"),
    address!("Cd5fE23C85820F7B72D0926FC9b05b43E359b7ee"),
];

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

    // Keep track of the block hash. We'll later validate the client's execution against this.
    let block_hash = host_executor.header.hash_slow();

    // Describes the call to the getRates function.
    let call = ContractInput {
        contract_address: CONTRACT,
        caller_address: CALLER,
        calldata: IOracleHelper::getRatesCall { collaterals: COLLATERALS.to_vec() },
    };

    // Call getRates from the host executor.
    let _rates = host_executor.execute(call).await?._0;

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
    let mut proof = client.prove(&pk, stdin).run().unwrap();
    println!("generated proof");

    // Read the block hash, and verify that it's the same as the one inputted.
    let client_block_hash = proof.public_values.read::<B256>();
    assert_eq!(client_block_hash, block_hash);

    // Read the output, in the form of a bunch of exchange rates.
    //
    // Note that this output is read from values commited to in the program using
    // `sp1_zkvm::io::commit`.
    let result = proof.public_values.read::<Vec<U256>>();
    println!("Got these rates: \n{:?}%", result);

    // Verify proof and public values.
    client.verify(&proof, &vk).expect("verification failed");
    println!("successfully generated and verified proof for the program!");
    Ok(())
}
