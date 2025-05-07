use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("ABI error: {0}")]
    ABI(#[from] alloy_sol_types::Error),
}
