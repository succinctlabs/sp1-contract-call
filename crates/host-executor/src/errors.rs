use alloy_eips::{eip2718::Eip2718Error, BlockId};
use alloy_transport::TransportError;
use rsp_mpt::FromProofError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HostError {
    #[error("Transport error: {0}")]
    TransportError(#[from] TransportError),
    #[error("Decoding error: {0}")]
    DecodingError(#[from] Eip2718Error),
    #[error("Trie from proof conversion error: {0}")]
    TrieFromProofError(#[from] FromProofError),
    #[error("Merkleization error: {0}")]
    MerkleizationError(#[from] ethereum_consensus::ssz::prelude::MerkleizationError),
    #[error("Beacon error: {0}")]
    BeaconError(#[from] BeaconError),
    #[error("Failed to convert the header for block {0}")]
    HeaderConversionError(u64),
    #[error("The block {0} don't exists")]
    BlockNotFoundError(BlockId),
    #[error("The parent beacon block root is missing in the header")]
    ParentBeaconBlockRootMissing,
}

#[derive(Error, Debug)]
pub enum BeaconError {
    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Execution payload missing")]
    ExecutionPayloadMissing,
}
