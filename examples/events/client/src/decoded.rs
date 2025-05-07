#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_primitives::U256;
use sp1_cc_client_executor::EventsInput;
use swap_events_client::IERC20;

pub fn main() {
    // Read the state sketch from stdin. Use this during the execution in order to
    // access Ethereum state.
    let events = sp1_zkvm::io::read::<EventsInput>();

    let sum = events
        .decoded_logs::<IERC20::Transfer>()
        .filter_map(|l| l.map(|l| l.data.value).ok())
        .sum::<U256>();

    sp1_zkvm::io::commit(&sum);
}
