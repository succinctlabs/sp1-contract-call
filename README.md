# SP1 Contract Calls

Generates zero-knowledge proofs of Ethereum smart contract execution. 

> [!CAUTION]
>
> This repository is not meant for production usage.

## Getting Started

To use SP1-contract-call, you must first have Rust installed and SP1 installed to build the client programs. Then, from the root directory of the repository, run 

```RUST_LOG=info cargo run --bin [example] --release``` 

where `[example]` is one of the following
* `uniswap`
    * Fetches the price of the UNI / WETH pair on Uniswap V3.
* `multiplexer`
    * Calls a contract that fetches the prices of many different collateral assets.
    * The source code of this contract is found in `examples/multiplexer/ZkOracleHelper.sol`.
    * Due to the size of this program, it's recommended to use the [SP1 Prover network](https://docs.succinct.xyz/generating-proofs/prover-network.html) to generate proofs for this example.


## Acknowledgments

* [Unstable.Money](https://www.unstable.money/): Developed the smart contract featured in the `multiplexer` example.
* [SP1](https://github.com/succinctlabs/sp1): A fast, feature-complete zkVM for developers that can prove the execution of arbitrary Rust (or any LLVM-compiled) program.