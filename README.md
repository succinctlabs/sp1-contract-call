# SP1 Contract Calls

Generates zero-knowledge proofs of Ethereum smart contract execution. 

> [!CAUTION]
>
> This repository is not meant for production usage.

## Overview

This library (`sp1-contract-call`, or `sp1-cc` for short), provides developers with a simple interface to efficiently generate a ZKP of Ethereum smart contract execution offchain, that can be verified cheaply onchain for ~280k gas. This enables developers to verifiably run very expensive Solidity smart contract calls and be able to use this information in their onchain applications. Developers simply specific their Solidity function interface in Rust using the [`alloy_sol_macro`](https://docs.rs/alloy-sol-macro/latest/alloy_sol_macro/) library and can write an SP1 program to generate these proofs. Let's check out an example below:

### Client

First, we create a Rust program that runs the Solidity smart contract call, using the `alloy_sol_macro` interface, the contract address and the caller address. This is known as a "client" program and it is run inside SP1 to generate a ZKP of the smart contract call's execution.

In this example, we use the `slot0` function to fetch the current price of the UNI/WETH pair on the UniswapV3 pool. Note that we abi encode the `public_values` -- this is to make it easy later to use those public values on chain. The code below is taken from [`examples/uniswap/client/src/main.rs`](./examples/uniswap/client/src/main.rs) which contains all of the code needed for the SP1 client program. 

```rs
sol! {
    /// Simplified interface of the IUniswapV3PoolState interface.
    interface IUniswapV3PoolState {
        function slot0(
        ) external view returns (uint160 sqrtPriceX96, ...);
    }
}

/// Address of Uniswap V3 pool.
const CONTRACT: Address = address!("1d42064Fc4Beb5F8aAF85F4617AE8b3b5B8Bd801");

...

let state_sketch_bytes = sp1_zkvm::io::read::<Vec<u8>>();
let state_sketch = bincode::deserialize::<EVMStateSketch>(&state_sketch_bytes).unwrap();

// Initialize the client executor with the state sketch.
// This step also validates all of the storage against the provided state root.
let executor = ClientExecutor::new(state_sketch).unwrap();

// Execute the slot0 call using the client executor.
let slot0_call = IUniswapV3PoolState::slot0Call {};
let call = ContractInput::new_call(CONTRACT, Address::default(), slot0_call);
let public_vals = executor.execute(call).unwrap();

// Commit the abi-encoded output.
sp1_zkvm::io::commit_slice(&public_vals.abi_encode());
```

### Host

Under the hood, the SP1 client program uses the executor from the `sp1-cc-client-executor` library, which requires storage slots and merkle proof information to correctly and verifiably run the smart contract execution.

The "host" program is code that is run outside of the zkVM & is responsible for fetching all of the witness data that is needed for the client program. This witness data includes storage slots, account information & merkle proofs that the client program verifies.

You can see in the host example code below that we run the exact same contract call with the host executor (instead of the client executor), and the host executor will fetch all relevant information as its executing. When we call `finalize()` on the host executor, it prepares all of the data it has gathered during contract call execution and then prepares it for input into the client program.

```rs
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
let _price_x96_bytes = host_executor
    .execute(ContractInput::new_call(CONTRACT, Address::default(), slot0_call))
    .await?;

// Now that we've executed all of the calls, get the `EVMStateSketch` from the host executor.
let input = host_executor.finalize().await?;

// Feed the sketch into the client.
let input_bytes = bincode::serialize(&input)?;
let mut stdin = SP1Stdin::new();
stdin.write(&input_bytes);

// Now we can call the client program.

...

```

After running the client program in the host, we generate a proof that can easily be verified on chain. In addition, the public values associated with our proof are abi-encoded, which allows us to use the output of the contract call on chain. Here is part of a sample contract that verifies this proof; check out [`examples/uniswap/contracts`](./examples/uniswap/contracts/) for more details. 

```sol
/// @title SP1 UniswapCall.
/// @notice This contract implements a simple example of verifying the proof of call to a smart 
///         contract.
contract UniswapCall {
    /// @notice The address of the SP1 verifier contract.
    /// @dev This can either be a specific SP1Verifier for a specific version, or the
    ///      SP1VerifierGateway which can be used to verify proofs for any version of SP1.
    ///      For the list of supported verifiers on each chain, see:
    ///      https://github.com/succinctlabs/sp1-contracts/tree/main/contracts/deployments
    address public verifier;

    /// @notice The verification key for the uniswapCall program.
    bytes32 public uniswapCallProgramVKey;

    constructor(address _verifier, bytes32 _uniswapCallProgramVKey) {
        verifier = _verifier;
        uniswapCallProgramVKey = _uniswapCallProgramVKey;
    }

    /// @notice The entrypoint for verifying the proof of a uniswapCall number.
    /// @param _proofBytes The encoded proof.
    /// @param _publicValues The encoded public values.
    function verifyUniswapCallProof(bytes calldata _publicValues, bytes calldata _proofBytes)
        public
        view
        returns (uint160)
    {
        ISP1Verifier(verifier).verifyProof(uniswapCallProgramVKey, _publicValues, _proofBytes);
        ContractPublicValues memory publicValues = abi.decode(_publicValues, (ContractPublicValues));
        uint160 sqrtPriceX96 = abi.decode(publicValues.contractOutput, (uint160));
        return sqrtPriceX96;
    }
}
```
## Running examples

To use SP1-contract-call, you must first have Rust installed and SP1 installed to build the client programs. In addition, you need to set the `ETH_RPC_URL` and `ETH_SEPOLIA_RPC_URL` environment variables. You can do this manually by running the following:

```
export ETH_RPC_URL=[YOUR ETH RPC URL HERE]
export ETH_SEPOLIA_RPC_URL=[YOUR ETH SEPOLIA RPC URL HERE]
``` 

Alternatively, you can use a `.env` file (see [example](./.env.example)).

Then, from the root directory of the repository, run 

```RUST_LOG=info cargo run --bin [example] --release``` 

where `[example]` is one of the following
* `uniswap`
    * Fetches the price of the UNI / WETH pair on Uniswap V3. By default, this does not generate a proof.  
    * Running `RUST_LOG=info cargo run --bin [example] --release -- --prove` will generate a plonk proof. This requires 
    significant computational resources, so we recommend using the [SP1 Prover network](https://docs.succinct.xyz/docs/generating-proofs/prover-network).
        * Outputs a file called [plonk-fixture.json](examples/uniswap/contracts/src/fixtures/plonk-fixture.json), which contains everything you need to verify the proof on chain. 
        * To see an example of on-chain verification, take a look at the [contracts](./examples/uniswap/contracts/) directory. 
* `multiplexer`
    * Calls a contract that fetches the prices of many different collateral assets.
    * The source code of this contract is found [here](./examples/multiplexer/ZkOracleHelper.sol).
    * Due to the size of this program, it's recommended to use the [SP1 Prover network](https://docs.succinct.xyz/docs/generating-proofs/prover-network) to generate proofs for this example.
* `verify-quorum`
    * Calls a contract that verifies several ECDSA signatures on chain, and sums the stake for the addresses corresponding to valid signatures.
* `example-deploy`
    * Demonstrates how to simulate a contract creation transaction on SP1-CC.

## Acknowledgments

* [Unstable.Money](https://www.unstable.money/): Developed the smart contract featured in the `multiplexer` example.
* [SP1](https://github.com/succinctlabs/sp1): A fast, feature-complete zkVM for developers that can prove the execution of arbitrary Rust (or any LLVM-compiled) program.
