use alloy_transport::TransportError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HostError {
    #[error("Transport error")]
    Transport(#[from] TransportError),
}
