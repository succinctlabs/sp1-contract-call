#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_primitives::{address, Address};
use alloy_sol_macro::sol;
use alloy_sol_types::SolValue;
use sp1_cc_client_executor::{io::EVMStateSketch, ClientExecutor, ContractInput};
sol! {
    /// Simplified interface of the IUniswapV3PoolState interface.
    interface IUniswapV3PoolState {
        function slot0(
        ) external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked);
    }
}

/// Address of Uniswap V3 pool.
const CONTRACT: Address = address!("1d42064Fc4Beb5F8aAF85F4617AE8b3b5B8Bd801");

pub fn main() {
    // Read the state sketch from stdin. Use this during the execution in order to
    // access Ethereum state.
    let state_sketch_bytes = sp1_zkvm::io::read::<Vec<u8>>();
    let state_sketch = bincode::deserialize::<EVMStateSketch>(&state_sketch_bytes).unwrap();

    // Initialize the client executor with the state sketch.
    // This step also validates all of the storage against the provided state root.
    let executor = ClientExecutor::new(&state_sketch).unwrap();

    // Execute the slot0 call using the client executor.
    let slot0_call = IUniswapV3PoolState::slot0Call {};
    let call = ContractInput::new_call(CONTRACT, Address::default(), slot0_call);
    let public_vals = executor.execute(call).unwrap();

    // Commit the abi-encoded output.
    sp1_zkvm::io::commit_slice(&public_vals.abi_encode());
}
