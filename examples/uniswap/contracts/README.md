# Uniswap sample verification contract

This contract demonstrates how to deserialize a proof, public values, and vkey from a json file, and use them for on chain verification. 

Make sure you have [foundry](https://github.com/foundry-rs/foundry) installed.

First, run the uniswap example with `cargo run --release --bin uniswap`. This serializes a proof, public values, and a vkey to [`plonk-fixture.json`](./src/fixtures/plonk-fixture.json).  

You can run the sample contract locally using `forge test -vvv`. This deserializes the relevant information from `plonk-fixture.json`, and verifies the proof using the SP1 verifier contract.