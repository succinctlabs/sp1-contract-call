use thiserror::Error;

/// Error types for the client library.
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("ABI error: {0}")]
    ABI(#[from] alloy_sol_types::Error),

    #[error("RSP error: {0}")]
    RSP(#[from] rsp_client_executor::error::ClientError),

    #[error("The logs weren't prefetched")]
    LogsNotPrefetched,

    #[error("The provided chain config is invalid")]
    InvalidChainConfig,
}
