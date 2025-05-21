---
title: Introduction
sidebar_position: 1
---

# SP1 Contract Call

This library (`sp1-contract-call`, or `sp1-cc` for short), provides developers with a simple interface to efficiently generate a ZKP of Ethereum smart contract execution offchain, that can be verified cheaply onchain for ~280k gas.

This enables developers to verifiably run very expensive Solidity smart contract calls and be able to use this information in their onchain applications. Developers simply specify their Solidity function interface in Rust using the [`alloy_sol_macro`](https://docs.rs/alloy-sol-macro/latest/alloy_sol_macro/) library and can write an SP1 program to generate these proofs.
