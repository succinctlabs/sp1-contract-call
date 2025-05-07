use std::collections::HashMap;

use alloy_primitives::{BlockHash, Log, TxHash};
use alloy_rpc_types::Log as RpcLog;
use serde::{Deserialize, Serialize};

/// Input to event logs with all block and tx metadata.
///
/// This object can be useful if you need log metadata (like block hash or rtx index),
/// or if you want to iterate on logs grouped by blocks or transactions.
///
/// *Note*: If you only need to interact with decoded events, you can use [`crate::EventsInput`]
/// That is more efficient.
#[derive(Debug, Serialize, Deserialize)]
pub struct LogsInput {
    logs: HashMap<BlockKey, HashMap<TransactionKey, Vec<LogContainer>>>,
}

impl LogsInput {
    /// Creates a new `LogsInput` from a list of RPC logs.
    pub fn new(logs: Vec<RpcLog>) -> Self {
        let mut logs_map =
            HashMap::<BlockKey, HashMap<TransactionKey, Vec<LogContainer>>, _>::new();

        for log in &logs {
            logs_map.entry(log.into()).or_default().entry(log.into()).or_default().push(log.into());
        }

        Self { logs: logs_map }
    }

    /// Retrieves an iterator over all logs in the input.
    pub fn logs(&self) -> impl Iterator<Item = RpcLog> + use<'_> {
        self.logs.iter().flat_map(|(block_key, txs)| {
            txs.iter()
                .flat_map(|(tx_key, logs)| logs.iter().map(|log| build_log(block_key, tx_key, log)))
        })
    }

    /// Retrieves logs at a specific block hash.
    pub fn logs_at_block_hash(
        &self,
        block_hash: BlockHash,
    ) -> impl Iterator<Item = RpcLog> + use<'_> {
        self.logs
            .iter()
            .filter_map(move |(block_key, txs)| {
                if block_key.hash.map(|h| h == block_hash).unwrap_or_default() {
                    Some(txs.iter().flat_map(|(tx_key, logs)| {
                        logs.iter().map(|log| build_log(block_key, tx_key, log))
                    }))
                } else {
                    None
                }
            })
            .flatten()
    }

    /// Retrieves logs at a specific block number.
    pub fn logs_at_block_number(
        &self,
        block_number: u64,
    ) -> impl Iterator<Item = RpcLog> + use<'_> {
        self.logs
            .iter()
            .filter_map(move |(block_key, txs)| {
                if block_key.number.map(|h| h == block_number).unwrap_or_default() {
                    Some(txs.iter().flat_map(|(tx_key, logs)| {
                        logs.iter().map(|log| build_log(block_key, tx_key, log))
                    }))
                } else {
                    None
                }
            })
            .flatten()
    }

    /// Retrieves logs for a specific transaction hash.
    pub fn logs_at_tx(&self, tx_hash: TxHash) -> impl Iterator<Item = RpcLog> + use<'_> {
        self.logs.iter().flat_map(move |(block_key, txs)| {
            txs.iter()
                .filter_map(move |(tx_key, logs)| {
                    if tx_key.hash.map(|h| h == tx_hash).unwrap_or_default() {
                        Some(logs.iter().map(|log| build_log(block_key, tx_key, log)))
                    } else {
                        None
                    }
                })
                .flatten()
        })
    }

    /// Returns an iterator over all block hashes.
    pub fn block_hashes(&self) -> impl Iterator<Item = BlockHash> + use<'_> {
        self.logs.iter().filter_map(|(k, _)| k.hash)
    }

    /// Returns an iterator over all block numbers.
    pub fn block_numbers(&self) -> impl Iterator<Item = u64> + use<'_> {
        self.logs.iter().filter_map(|(k, _)| k.number)
    }

    /// Returns an iterator over all transactions hashes.
    pub fn tx_hashes(&self) -> impl Iterator<Item = TxHash> + use<'_> {
        self.logs.iter().map(|(_, txs)| txs.iter().filter_map(|(k, _)| k.hash)).flatten()
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
struct BlockKey {
    hash: Option<BlockHash>,
    #[serde(with = "alloy_serde::quantity::opt")]
    number: Option<u64>,
    #[serde(with = "alloy_serde::quantity::opt")]
    timestamp: Option<u64>,
}

impl From<&RpcLog> for BlockKey {
    fn from(value: &RpcLog) -> Self {
        Self {
            hash: value.block_hash,
            number: value.block_number,
            timestamp: value.block_timestamp,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
struct TransactionKey {
    hash: Option<TxHash>,
    #[serde(with = "alloy_serde::quantity::opt")]
    index: Option<u64>,
}

impl From<&RpcLog> for TransactionKey {
    fn from(value: &RpcLog) -> Self {
        Self { hash: value.transaction_hash, index: value.transaction_index }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct LogContainer {
    log: Log,
    index: Option<u64>,
    removed: bool,
}

impl From<&RpcLog> for LogContainer {
    fn from(value: &RpcLog) -> Self {
        Self { log: value.inner.clone(), index: value.log_index, removed: value.removed }
    }
}

fn build_log(block_key: &BlockKey, tx_key: &TransactionKey, log: &LogContainer) -> RpcLog {
    RpcLog {
        inner: log.log.clone(),
        block_hash: block_key.hash,
        block_number: block_key.number,
        block_timestamp: block_key.timestamp,
        transaction_hash: tx_key.hash,
        transaction_index: tx_key.index,
        log_index: log.index,
        removed: log.removed,
    }
}
