---
title: RPC Configuration
sidebar_position: 3
---

## RPC Node Requirements

SP1 Contract Call fetches block and state data from a JSON-RPC node. You must use an archive node which preserves historical intermediate trie nodes needed for fetching storage proofs.

In Geth, the archive mode can be enabled with the `--gcmode=archive` option. You can also use an RPC provider that offers archive data access.

:::warning

Some RPC providers have issues with `eth_getProof` on older blocks. For instance QuickNode returns invalid data that lead to state mismatch errors. We recommend using [Alchemy](https://www.alchemy.com/).

:::

## Troubleshooting

### State root mismatch

This issue can be caused using an RPC provider that returns incorrect results from the `eth_getProof` endpoint. We have empirically observed such issues with many RPC providers. We recommend using [Alchemy](https://www.alchemy.com/).