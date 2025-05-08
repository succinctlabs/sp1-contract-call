use alloy_eips::eip2718::Eip2718Error;
use alloy_transport::TransportError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HostError {
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),
    #[error("Decoding error: {0}")]
    Decoding(#[from] Eip2718Error),
}
