#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_primitives::{address, Address};
use alloy_sol_macro::sol;
use sp1_cc_client_executor::{io::EvmSketchInput, ClientExecutor, ContractInput, Genesis};
sol! {
    /// Simplified interface of the IUniswapV3PoolState interface.
    interface IUniswapV3PoolState {
        function slot0(
        ) external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked);
    }
}

/// Address of Uniswap V3 pool.
const MAINNET_POOL_CONTRACT: Address = address!("1d42064Fc4Beb5F8aAF85F4617AE8b3b5B8Bd801");
const SEPOLIA_POOL_CONTRACT: Address = address!("3289680dD4d6C10bb19b899729cda5eEF58AEfF1");

pub fn main() {
    // Read the state sketch from stdin. Use this during the execution in order to
    // access Ethereum state.
    let state_sketch_bytes = sp1_zkvm::io::read::<Vec<u8>>();
    let state_sketch = bincode::deserialize::<EvmSketchInput>(&state_sketch_bytes).unwrap();

    let pool_contract = match state_sketch.genesis {
        Genesis::Mainnet => MAINNET_POOL_CONTRACT,
        Genesis::Sepolia => SEPOLIA_POOL_CONTRACT,
        _ => unimplemented!(),
    };

    // Initialize the client executor with the state sketch.
    // This step also validates all of the storage against the provided state root.
    let executor = ClientExecutor::eth(&state_sketch).unwrap();

    let slot0_call = IUniswapV3PoolState::slot0Call {};
    let call = ContractInput::new_call(pool_contract, Address::default(), slot0_call);
    // Execute the slot0 call using the client executor and commit the abi-encoded output.
    executor.execute_and_commit(call);
}
