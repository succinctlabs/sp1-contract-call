use alloy_primitives::{address, Address};
use alloy_provider::RootProvider;
use alloy_rpc_types::BlockNumberOrTag;
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use revm_primitives::{hex, Bytes};
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
    /// Simplified interface of the IUniswapV3PoolState interface.
    interface IUniswapV3PoolState {
        function slot0(
        ) external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked);
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

sol! {
    /// Part of the SimpleStaking interface
    interface SimpleStaking {
        function getStake(address addr) public view returns (uint256);
        function update(address addr, uint256 weight) public;
        function verifySigned(bytes32[] memory messageHashes, bytes[] memory signatures) public view returns (uint256);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_multiplexer() -> eyre::Result<()> {
    let get_rates_call = getRatesCall { collaterals: COLLATERALS.to_vec() };

    let contract_input = ContractInput::new_call(
        address!("0A8c00EcFA0816F4f09289ac52Fcb88eA5337526"),
        Address::default(),
        get_rates_call,
    );

    let public_values = test_e2e(contract_input).await?;

    let rates = getRatesCall::abi_decode_returns(&public_values.contractOutput, true)?._0;

    println!("rates: {:?}", rates);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_uniswap() -> eyre::Result<()> {
    let slot0_call = IUniswapV3PoolState::slot0Call {};

    let contract_input = ContractInput::new_call(
        address!("1d42064Fc4Beb5F8aAF85F4617AE8b3b5B8Bd801"),
        Address::default(),
        slot0_call,
    );

    let public_values = test_e2e(contract_input).await?;

    let _price_x96_bytes =
        IUniswapV3PoolState::slot0Call::abi_decode_returns(&public_values.contractOutput, true)?
            .sqrtPriceX96;

    Ok(())
}

/// This test goes to the Wrapped Ether contract, and gets the name of the token.
/// This should always be "Wrapped Ether".
#[tokio::test(flavor = "multi_thread")]
async fn test_wrapped_eth() -> eyre::Result<()> {
    let name_call = nameCall {};
    let contract_input = ContractInput::new_call(
        address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        Address::default(),
        name_call,
    );

    let public_values = test_e2e(contract_input).await?;

    let name = nameCall::abi_decode_returns(&public_values.contractOutput, true)?._0;
    assert_eq!(name, String::from("Wrapped Ether"));

    Ok(())
}

/// This tests contract creation transactions.
#[tokio::test(flavor = "multi_thread")]
async fn test_contract_creation() -> eyre::Result<()> {
    // Load environment variables.
    dotenv::dotenv().ok();

    let bytecode = "0x6080604052348015600e575f5ffd5b50415f5260205ff3fe";

    // Get a recent blob to get the hash from.
    let block_number = BlockNumberOrTag::Safe;

    // Use `ETH_SEPOLIA_RPC_URL` to get all of the necessary state for the smart contract call.
    let rpc_url = std::env::var("ETH_SEPOLIA_RPC_URL")
        .unwrap_or_else(|_| panic!("Missing ETH_SEPOLIA_RPC_URL in env"));
    let provider = RootProvider::new_http(Url::parse(&rpc_url)?);
    let mut host_executor = HostExecutor::new(provider.clone(), block_number).await?;

    // Keep track of the block hash. Later, validate the client's execution against this.
    let bytes = hex::decode(bytecode).expect("Decoding failed");
    println!("Checking coinbase");
    let contract_input = ContractInput::new_create(Address::default(), Bytes::from(bytes));
    let _check_coinbase = host_executor.execute(contract_input).await?;
    Ok(())
}

/// Emulates the entire workflow of executing a smart contract call, without using SP1.
///
/// First, executes the smart contract call with the given [`ContractInput`] in the host executor.
/// After getting the [`EVMStateSketch`] from the host executor, executes the same smart contract   
/// call in the client executor.
async fn test_e2e(contract_input: ContractInput) -> eyre::Result<ContractPublicValues> {
    // Load environment variables.
    dotenv::dotenv().ok();

    // Which block transactions are executed on.
    let block_number = BlockNumberOrTag::Latest;

    // Prepare the host executor.
    //
    // Use `RPC_URL` to get all of the necessary state for the smart contract call.
    let rpc_url = std::env::var("ETH_RPC_URL").unwrap_or_else(|_| panic!("Missing RPC_URL"));
    let provider = RootProvider::new_http(Url::parse(&rpc_url)?);
    let mut host_executor = HostExecutor::new(provider.clone(), block_number).await?;

    let _contract_output = host_executor.execute(contract_input.clone()).await?;

    // Now that we've executed all of the calls, get the `EVMStateSketch` from the host executor.
    let state_sketch = host_executor.finalize().await?;

    let client_executor = ClientExecutor::new(&state_sketch)?;

    let public_values = client_executor.execute(contract_input)?;

    Ok(public_values)
}
