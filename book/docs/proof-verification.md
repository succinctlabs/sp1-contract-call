---
title: Proof verification
sidebar_position: 5
---

## Overview

SP1 Contract call retrieve the state for contract calls at a specific block (called the execution block). In order to guarantee that the proof accurately reflects the correct blockchain state at the specified execution block, data (called the `Anchor`) identifying this block are added to the inputs sent to the zkVM and included to the proof public inputs.

The anchor consists of an identifier that identifies the block and a hash that enables its verification. The method used to generate the anchor have a direct impact of the window between the execution block and the block on which the verify transaction is contained, as you can see in the table below:

| Method                              | Anchor Identifier | Anchor Hash | On-chain validation | Validation window |
|-------------------------------------|-------------------|-------------|---------------------|-------------------|
| [Block hash](#using-block-hash)     | Block number      | Block hash  | ✅                  | 256 blocks        |
| [Beacon root](#using-beacon-root)   | Timestamp         | Beacon root | ✅                  | 8191 blocks       |
| [Beacon root (chained)](#chaining)  | Timestamp         | Beacon root | ✅                  | Up to Cancun      |
| [Consensus](#using-consensus)       | Slot              | Beacon root | ❌                  | N/A               |

## Using block hash

This method uses the `blockhash` opcode to commit to a block hash. This gives 256 blocks (approximately 50 minutes) to create the proof and confirm that the validating transaction is included in a block.

## Using beacon root

This approach enables verification through the [EIP-4788](https://eips.ethereum.org/EIPS/eip-4788) beacon roots contract. By using this technique, the onchain proof validation window is extended to 8191 blocks (approximately 27 hours). The method requires a beacon API endpoint connection and can be activated by invoking [`EvmSketchBuilder::cl_rpc_url()`]:

```rust
let sketch = EvmSketch::builder()
    .at_block(block_number)
    .el_rpc_url(eth_rpc_url)
    .cl_rpc_url(beacon_rpc_url)
    .build()
    .await?;
```

### Chaining

The EIP-4788 anchor mechanism can be used to query view call state from blocks beyond the 8191 block limit by separating the anchor into two components: an execution block and a reference block. While the reference block acts as the anchor and must remain within the ~27 hour onchain validation timeframe, the execution block can extend significantly further into the past—covering periods of days, weeks, or even months.

These two blocks have an inherent relationship: the execution block must always be an ancestor of the reference block. By validating a chain of beacon block roots between these two blocks, you can prove that the execution block exists within the committed chain.

The validation process traces backward from the reference block to the execution block through sequential calls to the beacon roots contract. This verifies the integrity of view call data in the execution block by demonstrating that it's a canonical ancestor of the reference block. Once deployed onchain, successful anchor validation confirms the integrity of the reference block's block root.

The reference block may be specified while building the sketck, using [`EvmSketchBuilder::at_reference_block()`]:

```rust
let sketch = EvmSketch::builder()
    .at_block(block_number)
    .el_rpc_url(eth_rpc_url)
    .cl_rpc_url(beacon_rpc_url)
    .at_reference_block(reference_block_number)
    .build()
    .await?;
```

## Using consensus

A consensus anchor stores the beacon block root using its slot number as the index. This differs from the standard approach, which uses timestamp-based lookups for verification through the EIP-4788 beacon root contract at the execution layer.

Slot-based indexing is especially advantageous for verification systems that can directly access beacon chain state, including those that employ beacon light clients. This enables the commitment to be validated directly against the consensus layer state.

To enable consensus anchor, call [`EvmSketchBuilder::consensus()`] like in the example below:

```rust
let sketch = EvmSketch::builder()
    .at_block(block_number)
    .el_rpc_url(eth_rpc_url)
    .cl_rpc_url(beacon_rpc_url)
    .consensus()
    .build()
    .await?;
```

[`EvmSketchBuilder::cl_rpc_url()`]: pathname:///api/sp1_cc_host_executor/struct.EvmSketchBuilder.html#method.cl_rpc_url
[`EvmSketchBuilder::at_reference_block()`]: pathname:///api/sp1_cc_host_executor/struct.EvmSketchBuilder.html#method.at_reference_block
[`EvmSketchBuilder::consensus()`]: pathname:///api/sp1_cc_host_executor/struct.EvmSketchBuilder.html#method.consensus