#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_primitives::{address, Address};
use alloy_sol_macro::sol;
use alloy_sol_types::{SolCall, SolValue};
use bincode;
use sp1_cc_client_executor::{io::EVMStateSketch, ClientExecutor, ContractInput};
sol! {
    /// Simplified interface of the IUniswapV3PoolState interface.
    interface IUniswapV3PoolState {
        function slot0(
        ) external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked);
    }
}

sol! {
    struct UniswapOutput {
        address contractAddress;
        address callerAddress;
        bytes contractCallData;
        uint160 sqrtPriceX96;
        bytes32 blockHash;
    }
}

/// Address of Uniswap V3 pool.
const CONTRACT: Address = address!("1d42064Fc4Beb5F8aAF85F4617AE8b3b5B8Bd801");

/// Address of the caller.
const CALLER: Address = address!("0000000000000000000000000000000000000000");

pub fn main() {
    // Read the state sketch from stdin. Use this during the execution in order to
    // access Ethereum state.
    let state_sketch_bytes = sp1_zkvm::io::read::<Vec<u8>>();
    let state_sketch = bincode::deserialize::<EVMStateSketch>(&state_sketch_bytes).unwrap();

    // Compute the block hash.
    let block_hash = state_sketch.header.hash_slow();

    // Initialize the client executor with the state sketch.
    // This step also validates all of the storage against the provided state root.
    let executor = ClientExecutor::new(state_sketch).unwrap();

    // Execute the slot0 call using the client executor.
    let slot0_call = IUniswapV3PoolState::slot0Call {};
    let input = ContractInput {
        contract_address: CONTRACT,
        caller_address: CALLER,
        calldata: slot0_call.clone(),
    };
    let sqrt_price_x96 = executor.execute(input).unwrap().sqrtPriceX96;

    // ABI encode the output.
    let output = UniswapOutput {
        contractAddress: CONTRACT,
        callerAddress: CALLER,
        contractCallData: slot0_call.abi_encode().into(),
        sqrtPriceX96: sqrt_price_x96,
        blockHash: block_hash,
    };

    // Commit the output
    sp1_zkvm::io::commit_slice(&output.abi_encode());
}
