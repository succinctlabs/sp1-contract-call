# SP1 Contract Calls (under construction)

This library allows you to make view calls to Ethereum chains, verifiable through SP1. Define your Solidity method and arguments, and get some data back. 

TODO (more)

## Running

The code in `examples/erc20/client` and `examples/erc20/host` give an example -- we query the balance of a particular account's USDT. 

run `RUST_LOG=info cargo run --release` to see how much USDT the account at `9737100D2F42a196DE56ED0d1f6fF598a250E7E4` has on Sepolia. 
