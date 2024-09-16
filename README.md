# SP1 Contract Calls

Generates zero-knowledge proofs of Ethereum smart contract execution. 

> [!CAUTION]
>
> This repository is not meant for production usage.

## Overview

This library (`sp1-contract-call`, or `sp1-cc` for short), provides developers with a simple interface to efficiently generate a ZKP of Ethereum smart contract execution offchain, that can be verified cheaply onchain for ~280k gas. This enables developers to verifiably run very expensive Solidity smart contract calls and be able to use this information in their smart contracts. Developers simply specific their Solidity function interface in Rust using the [`alloy_sol_macro`](https://docs.rs/alloy-sol-macro/latest/alloy_sol_macro/) library and can write an SP1 program to generate these proofs. Let's check out an example below:

### Client

First, we create a Rust program that runs the Solidity smart contract call, using the `alloy_sol_macro` interface, the contract address and the caller address. This is known as a "client" program and it is run inside SP1 to generate a ZKP of the smart contract call's execution.

In this example, we use the `slot0` function to fetch the current price of the UNI/WETH pair on the UniswapV3 pool. Note that we abi encode the `public_values` -- this is to make it easy later to use those public values on chain. The code below is taken from `examples/uniswap/client/main.rs` which contains all of the code needed for the SP1 client program. 

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
let public_values = executor.execute(input).unwrap();

// Commit the abi-encoded output.
sp1_zkvm::io::commit_slice(&public_values.abi_encode());
```

### Host

Under the hood, the SP1 client program uses the executor from the `sp1_cc` library, which requires storage slots and merkle proof information to correctly and verifiably run the smart contract execution.

The "host" program is code that is run outside of the zkVM & is responsible for fetching all of the witness data that is needed for the client program. This witness data includes storage slots, account information & merkle proofs that the client program verifies.

You can see in the host example code below that we run the exact same contract call with the host executor (instead of the client executor), and the host executor will fetch all relevant information as its executing. When we call finalize() on the host executor, it prepares all of the data it has gathered during contract call execution and then prepares it for input into the client program.

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

After running the client program in the host, we generate a proof that can easily be verified on chain. In addition, the public values associated with our proof are abi-encoded. This allows us to use the output of the contract call on chain. The following sample contract demonstrates how you might verify the outcome of the Uniswap contract call.

```
contract SP1UniswapCC {
    // The SP1 verification key hash for the Uniswap contract call client program.
    bytes32 public uniswapVkeyHash;
    // The block hash we run the query at. 
    bytes32 public targetBlockHash;
    // The SP1 verifier contract.
    ISP1Verifier public verifier;

    constructor(
        bytes32 _uniswapVkeyHash,
        bytes32 _targetBlockHash,
        address _verifier
    ) {
        uniswapVkeyHash = _uniswapVkeyHash;
        targetBlockHash = _initialBlockHash;
        verifier = ISP1Verifier(_verifier);
    }

    function verifyUniswapCallProof(
        bytes calldata proof,
        bytes calldata publicValues
    ) public {
        ContractPublicValues contractPublicValues = abi.decode(publicValues, ContractPublicValues);

        // Require that the block hash from the public values matches the target block hash. 
        require(contractPublicValues.blockHash == targetBlockHash);

        // Verify the proof with the associated public values.
        verifier.verifyProof(uniswapVkeyHash, publicValues, proof);

        // Now, you can do something with the contractOutput -- an abi encoded exchange rate. 

    }
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
    * Due to the size of this program, it's recommended to use the [SP1 Prover network](https://docs.succinct.xyz/generating-proofs/prover-network.html) to generate proofs for this example.


## Acknowledgments

* [Unstable.Money](https://www.unstable.money/): Developed the smart contract featured in the `multiplexer` example.
* [SP1](https://github.com/succinctlabs/sp1): A fast, feature-complete zkVM for developers that can prove the execution of arbitrary Rust (or any LLVM-compiled) program.