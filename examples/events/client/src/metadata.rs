#![no_main]
sp1_zkvm::entrypoint!(main);

use sp1_cc_client_executor::LogsInput;

pub fn main() {
    let logs = sp1_zkvm::io::read::<LogsInput>();

    let block_with_most_transfers = logs
        .block_numbers()
        .map(|block_number| (block_number, logs.logs_at_block_number(block_number).count()))
        .max_by(|a, b| a.1.cmp(&b.1))
        .unwrap();

    println!("{}", block_with_most_transfers.0)
}
