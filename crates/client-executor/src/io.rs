use std::iter::once;

use reth_primitives::{EthPrimitives, Header};
use revm_primitives::{Address, Bytecode, HashMap, B256, U256};
use rsp_client_executor::io::WitnessInput;
use rsp_mpt::EthereumState;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

/// Information about how the contract executions accessed state, which is needed to execute the
/// contract in SP1.
///
/// Instead of passing in the entire state, only the state roots and merkle proofs
/// for the storage slots that were modified and accessed are passed in.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EVMStateSketch {
    /// The current block header.
    #[serde_as(as = "alloy_consensus::serde_bincode_compat::Header")]
    pub header: Header,
    /// The previous block headers starting from the most recent. These are used for calls to the
    /// blockhash opcode.
    #[serde_as(as = "Vec<alloy_consensus::serde_bincode_compat::Header>")]
    pub ancestor_headers: Vec<Header>,
    /// Current block's Ethereum state.
    pub state: EthereumState,
    /// Requests to account state and storage slots.
    pub state_requests: HashMap<Address, Vec<U256>>,
    /// Account bytecodes.
    pub bytecodes: Vec<Bytecode>,
}

impl WitnessInput<EthPrimitives> for EVMStateSketch {
    #[inline(always)]
    fn state(&self) -> &EthereumState {
        &self.state
    }

    #[inline(always)]
    fn state_anchor(&self) -> B256 {
        self.header.state_root
    }

    #[inline(always)]
    fn state_requests(&self) -> impl Iterator<Item = (&Address, &Vec<U256>)> {
        self.state_requests.iter()
    }

    #[inline(always)]
    fn bytecodes(&self) -> impl Iterator<Item = &Bytecode> {
        self.bytecodes.iter()
    }

    #[inline(always)]
    fn headers(&self) -> impl Iterator<Item = &Header> {
        once(&self.header).chain(self.ancestor_headers.iter())
    }
}
