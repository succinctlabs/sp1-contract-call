---
title: Custom chains
sidebar_position: 8
---

If you want to run SP1 Contract Call on a custom EVM chain, you can use the [`with_genesis()`] function while [building the `EvmSketch`](https://succinctlabs.github.io/sp1-contract-call/api/sp1_cc_host_executor/struct.EvmSketch.html#method.builder). The [`Genesis`] enum allows to specify a custom chain by using its genesis JSON.

:::tip

You can find examples of genesis JSON [here](https://github.com/succinctlabs/rsp/tree/main/bin/host/genesis).

:::





[`with_genesis()`]: pathname:///api/sp1_cc_host_executor/struct.EvmSketchBuilder.html#method.with_genesis
[`Genesis`]: pathname:///api/sp1_cc_host_executor/enum.Genesis.html