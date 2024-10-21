//! The following contact will be ephemerally deployed to retrieve a coinbase from a block.
//! This trick can be used to retrieve whatever on chain logic without needing to deploy a contract.
//! Just write the information that you want to retrieve on solidity in the constructor and return it.

use alloy::hex;
use alloy_primitives::{Address, Bytes};
use alloy_provider::ReqwestProvider;
use alloy_rpc_types::BlockNumberOrTag;
use alloy_sol_types::SolValue;
use sp1_cc_client_executor::ContractInput;
use sp1_cc_host_executor::HostExecutor;
use url::Url;

/// The following bytecode corresponds to the following solidity contract:
/// ```solidity
/// /**
///  * Contract that returns a coinbase
///  */
/// contract CoinbaseScrapper {
///     /**
///     * Returns the blobHash on index 0
///      */
///     constructor() {
///         assembly {
///             mstore(0, coinbase())
///             return(0, 0x20)
///         }
///     }
/// }
/// ```
const BYTECODE: &str = "0x6080604052348015600e575f5ffd5b50415f5260205ff3fe";

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Get a recent blob to get the hash from.
    let block_number = BlockNumberOrTag::Safe;

    // Use `ETH_SEPOLIA_RPC_URL` to get all of the necessary state for the smart contract call.
    let rpc_url = std::env::var("ETH_SEPOLIA_RPC_URL")
        .unwrap_or_else(|_| panic!("Missing ETH_SEPOLIA_RPC_URL in env"));
    let provider = ReqwestProvider::new_http(Url::parse(&rpc_url)?);
    let mut host_executor = HostExecutor::new(provider.clone(), block_number).await?;

    // Keep track of the block hash. Later, validate the client's execution against this.
    let bytes = hex::decode(BYTECODE).expect("Decoding failed");
    println!("Checking coinbase");
    let contract_input = ContractInput::new_create(Address::default(), Bytes::from(bytes));
    let check_coinbase = host_executor.execute(contract_input).await?;

    let decoded_address: Address = Address::abi_decode(&check_coinbase, true)?;

    println!("Coinbase address: {:?}", decoded_address);
    Ok(())
}
