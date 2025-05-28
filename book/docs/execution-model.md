---
title: Execution Model
sidebar_position: 4
---

This page explains how the SP1 Contract call system captures Ethereum Virtual Machine (EVM) state for verifiable execution within the SP1 zkVM. The EVM state capture mechanism ensures that smart contract calls can be reproduced deterministically in the zero-knowledge environment with cryptographic proofs of correctness.

The execution model follow the following steps:

1. [Prefetch all the data needed for the contract calls execution](#prefetch)
2. [Generate inputs object to be sent to the client](#inputs-generation)
3. [Execute the client and optinally generate a proof](#client-execution-and-proof-generation)
4. [Verify the proof on-chain](#on-chain-verification)

## Prefetch

The EVM state capture system is built around the [`EvmSketch`](pathname:///api/sp1_cc_host_executor/struct.EvmSketch.html) struct, which prefetches and organizes all data required to execute Ethereum smart contract calls and and retrieve events logs in the zkVM. The sketch acts as a bridge between the host environment (which has access to Ethereum RPC endpoints) and the client environment (which executes in the isolated zkVM).

:::tip

The `EvmSketch` struct can be configured with `EvmSketchBuilder`. You can have a look at `EvmSketch::Builder()`.

:::

One an `EvmSketch` is instanciated, the following methods can be called:

* The `call()` method executes smart contract functions and records all accessed accounts and storage slots.
* The `create()` method handles contract deployment transactions, tracking the bytecode and initialization parameters required for deterministic contract creation in the zkVM.
* The `get_logs()` method prefetches event logs matching specified filters.

## Inputs Generation

The `EvmSketch::finalize()` method transforms the accumulated state access data into a `EvmSketchInput` suitable for zkVM execution. This structure is serialized and passed to the client executor for deterministic re-execution in the zkVM environment.

## Client Execution and Proof Generation

Client Execution is the process of executing smart contract calls inside the SP1 zkVM to generate zero-knowledge proofs. It receives serialized Ethereum state information from the Host Executor and re-executes the same contract calls in a verifiable environment. This dual execution model ensures that the computation performed in the zkVM matches exactly what was executed on the host while producing cryptographic proofs of correctness.

Typically, the following steps are performed:

1. The `EvmSketchInput` is deserialized.
2. A `ClientExecutor` is created from the `EvmSketchInput`.
3. The actual contract calls within the zkVM environment are performed using the `execute()` method. Event logs can be retrieved using the `get_logs()` method.

The `execute()` method returns a `ContractPublicValues` struct containing all information needed for on-chain verification.

You can refer to the SP1 documentation for more details about [executing the program in the zkVM](https://docs.succinct.xyz/docs/sp1/generating-proofs/basics#executing-the-program) and [generating a proof](https://docs.succinct.xyz/docs/sp1/generating-proofs/basics#generating-the-proof).

## On-chain Verification

SP1 provides all the tooling required to verify proofs on chain. You can find more details in the [on-chain verification](https://docs.succinct.xyz/docs/sp1/verification/getting-started#generating-sp1-proofs-for-onchain-verification) page in the SP1 documentation.

In addition, the public values associated with our proof are abi-encoded, which allows to use the output of the contract call on chain. We also added a [`ContractCall`](https://github.com/succinctlabs/sp1-contracts/blob/main/contracts/src/v4.0.0-rc.3/utils/ContractCall.sol) library in [`sp1-contract` project](https://github.com/succinctlabs/sp1-contracts/pulls) to easily verify the public values on-chain. Check out [`examples/uniswap/contracts`](https://github.com/succinctlabs/sp1-contract-call/tree/main/examples/uniswap/contracts) for more details.
