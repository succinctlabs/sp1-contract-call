use alloy_primitives::{address, Address};
use alloy_provider::ReqwestProvider;
use alloy_rpc_types::BlockNumberOrTag;
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use sp1_cc_client_executor::{ClientExecutor, ContractInput, ContractPublicValues};
use url::Url;
use ERC20Basic::nameCall;
use IOracleHelper::getRatesCall;

use crate::HostExecutor;

sol! {
    /// Simplified interface of the ERC20Basic interface.
    interface ERC20Basic {
        function name() public constant returns (string memory);
    }
}

sol! {
    /// Interface to the multiplexer contract. It gets the exchange rates of many tokens, including
    /// apxEth, ankrEth, and pufEth.
    interface IOracleHelper {
        function getRates(address[] memory collaterals) external view returns (uint256[] memory);
    }
}

/// Multiplexer collateral addresses
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

#[tokio::test(flavor = "multi_thread")]
async fn test_multiplexer() -> eyre::Result<()> {
    let get_rates_call = getRatesCall { collaterals: COLLATERALS.to_vec() };

    let contract_input = ContractInput {
        contract_address: address!("0A8c00EcFA0816F4f09289ac52Fcb88eA5337526"),
        caller_address: Address::default(),
        calldata: get_rates_call,
    };

    let public_values = test_e2e(contract_input).await?;

    let rates = getRatesCall::abi_decode_returns(&public_values.contractOutput, true)?._0;

    println!("rates: {:?}", rates);

    Ok(())
}

/// This test goes to the Wrapped Ether contract, and gets the name of the token.
/// This should always be "Wrapped Ether".
#[tokio::test(flavor = "multi_thread")]
async fn test_wrapped_eth() -> eyre::Result<()> {
    let name_call = nameCall {};
    let contract_input = ContractInput {
        contract_address: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        caller_address: Address::default(),
        calldata: name_call,
    };
    let public_values = test_e2e(contract_input).await?;

    let name = nameCall::abi_decode_returns(&public_values.contractOutput, true)?._0;
    assert_eq!(name, String::from("Wrapped Ether"));

    Ok(())
}

/// Emulates the entire workflow of executing a smart contract call, without using SP1.
///
/// First, executes the smart contract call with the given [`ContractInput`] in the host executor.
/// After getting the [`EVMStateSketch`] from the host executor, executes the same smart contract   
/// call in the client executor.
async fn test_e2e<C: SolCall + Clone>(
    contract_input: ContractInput<C>,
) -> eyre::Result<ContractPublicValues> {
    // Load environment variables.
    dotenv::dotenv().ok();

    let mainnet = rsp_primitives::chain_spec::mainnet();

    // Which block transactions are executed on.
    let block_number = BlockNumberOrTag::Latest;

    // Prepare the host executor.
    //
    // Use `RPC_URL` to get all of the necessary state for the smart contract call.
    let rpc_url = std::env::var("ETH_RPC_URL").unwrap_or_else(|_| panic!("Missing RPC_URL"));
    let provider = ReqwestProvider::new_http(Url::parse(&rpc_url)?);
    let mut host_executor =
        HostExecutor::new(provider.clone(), block_number, mainnet.clone()).await?;

    let _contract_output = host_executor.execute(contract_input.clone()).await?;

    // Now that we've executed all of the calls, get the `EVMStateSketch` from the host executor.
    let state_sketch = host_executor.finalize().await?;

    let client_executor = ClientExecutor::new(state_sketch, mainnet.clone())?;

    let public_values = client_executor.execute(contract_input)?;

    Ok(public_values)
}
