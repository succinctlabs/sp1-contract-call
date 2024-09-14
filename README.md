# SP1 Contract Calls

Generates zero-knowledge proofs of Ethereum smart contract execution. 

> [!CAUTION]
>
> This repository is not meant for production usage.

## Overview

This repository will allow you to efficiently call Ethereum smart contracts off chain, while verifying the correctness of their execution using SP1. Previous verifiable methods to obtain Ethereum state off chain involved deep developer knowledge of Ethereum storage mechanisms, but `sp1-cc` allows you to specify your Solidity function interface natively in Rust, allowing for a seamless developer experience.

To illustrate how SP1 works, let's start with a simple example -- fetching the current price of the UNI / WETH pair on a UniswapV3 Pool. Here's a code snippet from `examples/uniswap/client` -- this code is ran in SP1.

```
sol! {
    /// Simplified interface of the IUniswapV3PoolState interface.
    interface IUniswapV3PoolState {
        function slot0(
        ) external view returns (uint160 sqrtPriceX96, ...);
    }
}

/// Address of Uniswap V3 pool.
const CONTRACT: Address = address!("1d42064Fc4Beb5F8aAF85F4617AE8b3b5B8Bd801");

/// Address of the caller.
const CALLER: Address = address!("0000000000000000000000000000000000000000");

...

// Execute the slot0 call using the client executor.
let slot0_call = IUniswapV3PoolState::slot0Call {};
let input =
    ContractInput { contract_address: CONTRACT, caller_address: CALLER, calldata: slot0_call };
let price_x96 = executor.execute(input).unwrap().sqrtPriceX96;
```

However, since we can't directly access the network in SP1, we need to execute a preflight call before  running this client code in SP1. The following excerpt from `examples/uniswap/host` demonstrates this.

```
...

// Prepare the host executor.
//
// Use `RPC_URL` to get all of the necessary state for the smart contract call.
let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| panic!("Missing RPC_URL"));
let provider = ReqwestProvider::new_http(Url::parse(&rpc_url)?);
let mut host_executor = HostExecutor::new(provider.clone(), block_number).await?;

// Keep track of the block hash. We'll later validate the client's execution against this.
let block_hash = host_executor.header.hash_slow();

// Make the call to the slot0 function.
let slot0_call = IUniswapV3PoolState::slot0Call {};
let _price_x96 = host_executor
    .execute(ContractInput {
        contract_address: CONTRACT,
        caller_address: CALLER,
        calldata: slot0_call,
    })
    .await?
    .sqrtPriceX96;

// Now that we've executed all of the calls, get the `EVMStateSketch` from the host executor.
let input = host_executor.finalize().await?;

// Feed the sketch into the client.
let input_bytes = bincode::serialize(&input)?;
let mut stdin = SP1Stdin::new();
stdin.write(&input_bytes);

// Now we can call the client program.

...

```

The host executor runs the contract, and keeps track of the state that contract accessed. It then feeds this state into the client. 

The resulting proof can then be verified on chain with a Solidity function similar to the following.

```
function verifyUniswapCallProof(
    bytes calldata proof,
    bytes calldata publicValues
) public {
		
    // In your application, you'll probably want to validate the committed block hash. 
    // Depending on your application, you might want to do some other checks on the public values.
    ContractOutput uniswapPublicValues = abi.decode(publicValues, ContractOutput);
    require(uniswapPublicValues.blockHash, myBlockHash)

    // Verify the proof with the associated public values.
    // Here, verifier is an instance of ISP1Verifier
    verifier.verifyProof(uniswapProgramVkeyHash, publicValues, proof);

    ...
}
```
## Running examples

To use SP1-contract-call, you must first have Rust installed and SP1 installed to build the client programs. In addition, you need to set the `ETH_RPC_URL` and `ETH_SEPOLIA_RPC_URL` environment variables. You can do this manually by running the following:

```
export ETH_RPC_URL=[YOUR ETH RPC URL HERE]
export ETH_SEPOLIA_RPC_URL=[YOUR ETH SEPOLIA RPC URL HERE]
``` 

Alternatively, you can use a `.env` file (see [example](./example.env)).

Then, from the root directory of the repository, run 

```RUST_LOG=info cargo run --bin [example] --release``` 

where `[example]` is one of the following
* `uniswap`
    * Fetches the price of the UNI / WETH pair on Uniswap V3.
* `multiplexer`
    * Calls a contract that fetches the prices of many different collateral assets.
    * The source code of this contract is found in `examples/multiplexer/ZkOracleHelper.sol`.


## Acknowledgments

* [Unstable.Money](https://www.unstable.money/): Developed the smart contract featured in the `multiplexer` example.
* [SP1](https://github.com/succinctlabs/sp1): A fast, feature-complete zkVM for developers that can prove the execution of arbitrary Rust (or any LLVM-compiled) program.