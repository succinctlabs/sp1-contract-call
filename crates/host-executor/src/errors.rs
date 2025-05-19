use alloy_eips::eip2718::Eip2718Error;
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
    #[error("Failed to convert the header for block {0}")]
    HeaderConversionError(u64),
}
