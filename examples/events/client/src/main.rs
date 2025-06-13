#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_rpc_types::Filter;
use alloy_sol_types::SolEvent;
use events_client::{IERC20, WETH};
use sp1_cc_client_executor::{io::EvmSketchInput, ClientExecutor};

pub fn main() {
    let state_sketch = sp1_zkvm::io::read::<EvmSketchInput>();

    // Initialize the client executor with the state sketch.
    // This step also validates all of the storage against the provided state root.
    let executor = ClientExecutor::eth(&state_sketch).unwrap();
    let filter = Filter::new().address(WETH).event(IERC20::Transfer::SIGNATURE);
    let logs = executor.get_logs::<IERC20::Transfer>(filter).unwrap();

    println!("WETH transfers: {}", logs.len())
}
