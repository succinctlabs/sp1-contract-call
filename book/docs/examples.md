---
title: Examples
sidebar_position: 8
---

## Uniswap (basic)

The Uniswap V3 integration example demonstrates fetching price data from Uniswap V3 pools and generating zero-knowledge proofs of the price queries. The example shows how to use the sp1-contract-call system to prove the execution of the slot0 function on Uniswap V3 pool contracts.

:::tip

This example can be ran with the following command:

```sh
RUST_LOG=info cargo run --bin uniswap-basic --release
```

You can add the `--prove` argument to generate a proof.

:::

:::warning

Running with `--prove` will generate a plonk proof. This requires significant computational resources, so we recommend using the [SP1 Prover network](https://docs.succinct.xyz/docs/network/developers/intro).

:::


## Uniswap (on-chain verify)

This example is similar to the basic Uniswap example above, with a few modification to demonstrate on-chain verification. The contract used to verify the proof and the public values can be found at the [contracts](https://github.com/succinctlabs/sp1-contract-call/tree/main/examples/uniswap/contracts) directory.

:::tip

This example can be ran with the following command:

```sh
RUST_LOG=info cargo run --bin uniswap-onchain-verify --release
```
By default, the `blockhash()` opcode is used, allowing to verify up to 256 blocks old, but the following arguments can be added to demonstrate the various features abaliable:

* If you provides a Beacon RPC endpoint with the `--beacon-sepolia-rpc-url` argument, the proof will be verified on chain with the beacon root using [EIP-4788](https://eips.ethereum.org/EIPS/eip-4788), up to 8191 blocks old (~27h).
* The window can even be extended up to the Cancun hardfork by chaining beacon roots using the `--reference-block` argument. 

:::

:::warning

This example will generate a plonk proof. This requires significant computational resources, so we recommend using the [SP1 Prover network](https://docs.succinct.xyz/docs/network/developers/intro).

:::


## Multiplexer

The Multiplexer Oracle example demonstrates fetching exchange rates for multiple collateral tokens from an on-chain oracle contract and generating zero-knowledge proofs of the retrieved data. 

:::tip

This example can be ran with the following command:

```sh
RUST_LOG=info cargo run --bin multiplexer --release
```

:::

## Verify quorum

The quorum verification example demonstrates how to prove ECDSA signature validation within the SP1 zkVM. The example sums the stake for the addresses corresponding to valid signatures.

:::tip

This example can be ran with the following command:

```sh
RUST_LOG=info cargo run --bin verify-quorum --release
```

:::

## Deploy

The deploy example demonstrates how to simulate a contract creation transaction on SP1 Contract Call.

:::tip

This example can be ran with the following command:

```sh
RUST_LOG=info cargo run --bin example-deploy --release
```

:::

## Events

The event processing example demonstrates how to fetch, filter, and verify Ethereum event logs using zero-knowledge proofs, providing trustless access to blockchain event data.

:::tip

This example can be ran with the following command:

```sh
RUST_LOG=info cargo run --bin events --release
```

:::