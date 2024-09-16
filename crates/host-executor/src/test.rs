use alloy_primitives::{address, Address, U160};
use alloy_provider::ReqwestProvider;
use alloy_rpc_types::BlockNumberOrTag;
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use sp1_cc_client_executor::{ClientExecutor, ContractInput};
use url::Url;
use IUniswapV3PoolState::slot0Call;

use crate::HostExecutor;

sol! {
    /// Simplified interface of the IUniswapV3PoolState interface.
    interface IUniswapV3PoolState {
        function slot0() external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked);
    }
}

/// Address of Uniswap V3 pool.
const CONTRACT: Address = address!("1d42064Fc4Beb5F8aAF85F4617AE8b3b5B8Bd801");

/// Address of the caller.
const CALLER: Address = address!("0000000000000000000000000000000000000000");

#[tokio::test(flavor = "multi_thread")]
async fn test_e2e() -> eyre::Result<()> {
    dotenv::dotenv().ok();

    // Which block transactions are executed on.
    let block_number = BlockNumberOrTag::Number(20764847);

    // Prepare the host executor.
    //
    // Use `RPC_URL` to get all of the necessary state for the smart contract call.
    let rpc_url = std::env::var("ETH_RPC_URL").unwrap_or_else(|_| panic!("Missing RPC_URL"));
    let provider = ReqwestProvider::new_http(Url::parse(&rpc_url)?);
    let mut host_executor = HostExecutor::new(provider.clone(), block_number).await?;

    // Make the call to the slot0 function.
    let slot0_call = slot0Call {};
    let contract_input =
        ContractInput { contract_address: CONTRACT, caller_address: CALLER, calldata: slot0_call };
    let _price_x96 = host_executor.execute(contract_input.clone()).await?.sqrtPriceX96;

    // Now that we've executed all of the calls, get the `EVMStateSketch` from the host executor.
    let state_sketch = host_executor.finalize().await?;

    let client_executor = ClientExecutor::new(state_sketch)?;

    let public_values = client_executor.execute(contract_input)?;

    // Read the output, and then calculate the uniswap exchange rate.
    //
    // Note that this output is read from values commited to in the program using
    // `sp1_zkvm::io::commit`.
    let sqrt_price_x96 =
        slot0Call::abi_decode_returns(&public_values.contractOutput, true)?.sqrtPriceX96;

    assert_eq!(sqrt_price_x96, U160::from(4173033185634422243650260000u128));

    Ok(())
}
