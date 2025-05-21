---
title: Getting Started
sidebar_position: 2
---

This guide will walk you through using the SP1 contract call framework with a practical Uniswap example. You'll learn how to generate zero-knowledge proofs of on-chain contract executions by querying price data from a Uniswap V3 pool. The example demonstrates both the client program (running inside SP1) and the host program needed to generate and verify proofs of contract execution.

## Client

First, we create a Rust program that runs the Solidity smart contract call, using the `alloy_sol_macro` interface, the contract address and the caller address. This is known as a "client" program and it is run inside SP1 to generate a ZKP of the smart contract call's execution.

In this example, we use the `slot0` function to fetch the current price of the UNI/WETH pair on the UniswapV3 pool. Note that we abi encode the `public_values` - this is to make it easy later to use those public values on chain. The code below is taken from [`examples/uniswap/client/src/main.rs`](./examples/uniswap/client/src/main.rs) which contains all of the code needed for the SP1 client program.

```rust
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
let state_sketch = bincode::deserialize::<EvmSketchInput>(&state_sketch_bytes).unwrap();

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

## Host

Under the hood, the SP1 client program uses the executor from the `sp1-cc-client-executor` library, which requires storage slots and merkle proof information to correctly and verifiably run the smart contract execution.

The "host" program is code that is run outside of the zkVM & is responsible for fetching all of the witness data that is needed for the client program. This witness data includes storage slots, account information & merkle proofs that the client program verifies.

You can see in the host example code below that we run the exact same contract call with the host executor (instead of the client executor), and the host executor will fetch all relevant information as its executing. When we call `finalize()` on the host executor, it prepares all of the data it has gathered during contract call execution and then prepares it for input into the client program.

```rust
...

// Prepare the host executor.
//
// Use `RPC_URL` to get all of the necessary state for the smart contract call.
let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| panic!("Missing RPC_URL"));
let sketch = EvmSketch::builder()
    .at_block(block_number)
    .el_rpc_url(rpc_url.parse()?)
    .build()
    .await?;

// Keep track of the block hash. We'll later validate the client's execution against this.
let block_hash = sketch.anchor.resolve().hash;;

// Make the call to the slot0 function.
let slot0_call = IUniswapV3PoolState::slot0Call {};
let _price_x96_bytes = sketch
    .call(ContractInput::new_call(CONTRACT, Address::default(), slot0_call))
    .await?;

// Now that we've executed all of the calls, get the `EVMStateSketch` from the host executor.
let input = sketch.finalize().await?;

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
