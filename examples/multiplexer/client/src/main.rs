#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_primitives::{address, Address};
use alloy_sol_macro::sol;
use alloy_sol_types::{SolCall, SolValue};
use bincode;
use sp1_cc_client_executor::{
    io::EVMStateSketch, ClientExecutor, ContractInput, ContractPublicValues,
};

sol! {
    /// Interface to the multiplexer contract. It gets the prices of many tokens, including
    /// apxEth, ankrEth, pufEth, and more.
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

pub fn main() {
    // Read the state sketch from stdin. Use this during the execution in order to
    // access Ethereum state.
    let state_sketch_bytes = sp1_zkvm::io::read::<Vec<u8>>();
    let state_sketch = bincode::deserialize::<EVMStateSketch>(&state_sketch_bytes).unwrap();

    // Compute the sketch's timestamp and block height.
    let timestamp = state_sketch.header.timestamp;
    let block_number = state_sketch.header.number;

    // Initialize the client executor with the state sketch.
    // This step also validates all of the storage against the provided state root.
    let executor = ClientExecutor::new(state_sketch).unwrap();

    // Execute the getRates call using the client executor.
    let calldata = IOracleHelper::getRatesCall { collaterals: COLLATERALS.to_vec() };
    let call = ContractInput {
        contract_address: CONTRACT,
        caller_address: CALLER,
        calldata: calldata.clone(),
    };
    let contract_public_values = executor.execute(call).unwrap();

    // Commit the abi-encoded output.
    sp1_zkvm::io::commit_slice(&contract_public_values.abi_encode());
}
