use alloy_primitives::{address, Address};
use alloy_provider::ReqwestProvider;
use alloy_rpc_types::BlockNumberOrTag;
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use revm_primitives::U256;
use sp1_cc_client_executor::{ClientExecutor, ContractInput};
use url::Url;
use ERC20Basic::totalSupplyCall;

use crate::HostExecutor;

sol! {
    /// Simplified interface of the ERC20Basic interface.
    interface ERC20Basic {
        function totalSupply() public constant returns (uint);
    }
}

/// Address of Uniswap V3 pool.
const CONTRACT: Address = address!("dAC17F958D2ee523a2206206994597C13D831ec7");

/// Address of the caller.
const CALLER: Address = address!("0000000000000000000000000000000000000000");

#[tokio::test(flavor = "multi_thread")]
async fn test_e2e() -> eyre::Result<()> {
    dotenv::dotenv().ok();

    // Which block transactions are executed on.
    let block_number = BlockNumberOrTag::Latest;

    // Prepare the host executor.
    //
    // Use `RPC_URL` to get all of the necessary state for the smart contract call.
    let rpc_url = std::env::var("ETH_RPC_URL").unwrap_or_else(|_| panic!("Missing RPC_URL"));
    let provider = ReqwestProvider::new_http(Url::parse(&rpc_url)?);
    let mut host_executor = HostExecutor::new(provider.clone(), block_number).await?;

    // Make the call to the slot0 function.
    let total_supply_call = totalSupplyCall {};
    let contract_input = ContractInput {
        contract_address: CONTRACT,
        caller_address: CALLER,
        calldata: total_supply_call,
    };
    let _total_supply = host_executor.execute(contract_input.clone()).await?._0;

    // Now that we've executed all of the calls, get the `EVMStateSketch` from the host executor.
    let state_sketch = host_executor.finalize().await?;

    let client_executor = ClientExecutor::new(state_sketch)?;

    let public_values = client_executor.execute(contract_input)?;

    // Read the output, and then calculate the uniswap exchange rate.
    //
    // Note that this output is read from values commited to in the program using
    // `sp1_zkvm::io::commit`.
    let total_supply = totalSupplyCall::abi_decode_returns(&public_values.contractOutput, true)?._0;

    // println!("total_supply: {}", total_supply);
    assert_eq!(total_supply, U256::from(54981730120396390u128));

    Ok(())
}
