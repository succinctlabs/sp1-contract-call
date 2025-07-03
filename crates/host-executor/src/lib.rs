//! SP1 Contract Call Host Executor Library
//!
//! This library provides functionality for executing contract calls within the SP1 zkVM host
//! environment.

pub use rsp_primitives::genesis::Genesis;

mod anchor_builder;
pub use anchor_builder::{
    AnchorBuilder, BeaconAnchorBuilder, BeaconAnchorKind, BeaconBlockField,
    ChainedBeaconAnchorBuilder, ConsensusBeaconAnchor, Eip4788BeaconAnchor, HeaderAnchorBuilder,
};

mod beacon;
pub use beacon::BeaconClient;

mod errors;
pub use errors::{BeaconError, HostError};

mod sketch;
pub use sketch::EvmSketch;

mod sketch_builder;
pub use sketch_builder::EvmSketchBuilder;

#[cfg(test)]
mod test;
