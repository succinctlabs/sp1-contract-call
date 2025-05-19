use std::{collections::HashMap, fmt::Display};

use alloy_consensus::Header;
use alloy_eips::eip4788::BEACON_ROOTS_ADDRESS;
use alloy_primitives::{uint, B256, U256};
use revm::DatabaseRef;
use rsp_client_executor::io::TrieDB;
use rsp_mpt::EthereumState;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sha2::{Digest, Sha256};

use crate::AnchorType;

// https://eips.ethereum.org/EIPS/eip-4788
pub const HISTORY_BUFFER_LENGTH: U256 = uint!(8191_U256);
/// The generalized Merkle tree index of the `block_hash` field in the `BeaconBlock`.
const BLOCK_HASH_LEAF_INDEX: usize = 6444;
/// The generalized Merkle tree index of the `state_root` field in the `BeaconBlock`.
const STATE_ROOT_LEAF_INDEX: usize = 6434;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Anchor {
    Header(HeaderAnchor),
    Beacon(BeaconWithHeaderAnchor),
    Chained(ChainedBeaconAnchor),
}

impl Anchor {
    pub fn header(&self) -> &Header {
        match self {
            Anchor::Header(header_anchor) => &header_anchor.header,
            Anchor::Beacon(beacon_anchor) => &beacon_anchor.inner.header,
            Anchor::Chained(chained_anchor) => &chained_anchor.inner.inner.header,
        }
    }

    pub fn resolve(&self) -> ResolvedAnchor {
        match self {
            Anchor::Header(header_anchor) => ResolvedAnchor {
                id: U256::from(header_anchor.header.number),
                hash: header_anchor.header.hash_slow(),
            },
            Anchor::Beacon(beacon_anchor) => {
                let block_hash = beacon_anchor.inner.header.hash_slow();
                let hash = beacon_anchor.anchor.beacon_root(block_hash, BLOCK_HASH_LEAF_INDEX);

                ResolvedAnchor { id: beacon_anchor.id().into(), hash }
            }
            Anchor::Chained(chained_anchor) => {
                // Retrieve the execution block beacon root and timestamp
                let mut beacon_root = chained_anchor.inner.beacon_root();
                let mut timestamp = U256::from(chained_anchor.inner.id().as_timestamp().unwrap());

                // Iterate over all the state anchors stating from the execution block
                // to the reference block
                for state_anchor in &chained_anchor.state_anchors {
                    let state_root = state_anchor.state.state_root();
                    let current_beacon_root =
                        get_beacon_root_from_state(&state_anchor.state, timestamp);

                    // Verify that the previous anchor is valid wrt the current state
                    assert_eq!(current_beacon_root, beacon_root, "Beacon root should match");

                    // Retrieve the beacon root and timestamp of the current state
                    beacon_root =
                        state_anchor.anchor.beacon_root(state_root, STATE_ROOT_LEAF_INDEX);
                    timestamp = U256::from(state_anchor.anchor.id().as_timestamp().unwrap());
                }

                // If the full chain is valid, return the resolved anchor containing
                // the reference block beacon root and timestamp
                ResolvedAnchor { id: timestamp, hash: beacon_root }
            }
        }
    }

    pub fn ty(&self) -> AnchorType {
        match self {
            Anchor::Header(_) => AnchorType::BlockHash,
            Anchor::Beacon(_) | Anchor::Chained(_) => AnchorType::BeaconRoot,
        }
    }
}

impl From<Header> for Anchor {
    fn from(header: Header) -> Self {
        Self::Header(HeaderAnchor { header })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ResolvedAnchor {
    pub id: U256,
    pub hash: B256,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeaderAnchor {
    #[serde_as(as = "alloy_consensus::serde_bincode_compat::Header")]
    header: Header,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BeaconWithHeaderAnchor {
    inner: HeaderAnchor,
    anchor: BeaconAnchor,
}

impl BeaconWithHeaderAnchor {
    pub fn new(header: Header, anchor: BeaconAnchor) -> Self {
        Self { inner: HeaderAnchor { header }, anchor }
    }

    pub fn proof(&self) -> &[B256] {
        self.anchor.proof()
    }

    pub fn id(&self) -> &BeaconAnchorId {
        self.anchor.id()
    }

    pub fn beacon_root(&self) -> B256 {
        self.anchor.beacon_root(self.inner.header.hash_slow(), BLOCK_HASH_LEAF_INDEX)
    }
}

impl From<BeaconWithHeaderAnchor> for BeaconAnchor {
    fn from(value: BeaconWithHeaderAnchor) -> Self {
        value.anchor
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BeaconAnchor {
    proof: Vec<B256>,
    id: BeaconAnchorId,
}

impl BeaconAnchor {
    pub fn new(proof: Vec<B256>, id: BeaconAnchorId) -> Self {
        Self { proof, id }
    }

    pub fn proof(&self) -> &[B256] {
        &self.proof
    }

    pub fn id(&self) -> &BeaconAnchorId {
        &self.id
    }

    pub fn beacon_root(&self, leaf: B256, generalized_index: usize) -> B256 {
        rebuild_merkle_root(leaf, generalized_index, &self.proof)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BeaconAnchorId {
    Timestamp(u64),
    Slot(u64),
}

impl BeaconAnchorId {
    pub fn as_timestamp(&self) -> Option<u64> {
        match self {
            BeaconAnchorId::Timestamp(t) => Some(*t),
            BeaconAnchorId::Slot(_) => None,
        }
    }
}

impl From<&BeaconAnchorId> for U256 {
    fn from(value: &BeaconAnchorId) -> Self {
        match value {
            BeaconAnchorId::Timestamp(t) => U256::from(*t),
            BeaconAnchorId::Slot(s) => U256::from(*s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChainedBeaconAnchor {
    inner: BeaconWithHeaderAnchor,
    state_anchors: Vec<BeaconStateAnchor>,
}

impl ChainedBeaconAnchor {
    pub fn new(inner: BeaconWithHeaderAnchor, state_anchors: Vec<BeaconStateAnchor>) -> Self {
        Self { inner, state_anchors }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BeaconStateAnchor {
    state: EthereumState,
    anchor: BeaconAnchor,
}

impl BeaconStateAnchor {
    pub fn new(state: EthereumState, anchor: BeaconAnchor) -> Self {
        Self { state, anchor }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BeaconBlockField {
    BlockHash,
    StateRoot,
}

impl Display for BeaconBlockField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BeaconBlockField::BlockHash => write!(f, "block_hash"),
            BeaconBlockField::StateRoot => write!(f, "state_root"),
        }
    }
}

impl PartialEq<BeaconBlockField> for usize {
    fn eq(&self, other: &BeaconBlockField) -> bool {
        let other = usize::from(other);

        *self == other
    }
}

impl From<&BeaconBlockField> for usize {
    fn from(value: &BeaconBlockField) -> Self {
        match value {
            BeaconBlockField::BlockHash => BLOCK_HASH_LEAF_INDEX,
            BeaconBlockField::StateRoot => STATE_ROOT_LEAF_INDEX,
        }
    }
}

pub fn rebuild_merkle_root(leaf: B256, generalized_index: usize, branch: &[B256]) -> B256 {
    let mut current_hash = leaf;
    let depth = generalized_index.ilog2();
    let mut index = generalized_index - (1 << depth);
    let mut hasher = Sha256::new();

    for sibling in branch {
        // Determine if the current node is a left or right child
        let is_left = index % 2 == 0;

        // Combine the current hash with the sibling hash
        if is_left {
            // If current node is left child, hash(current + sibling)
            hasher.update(current_hash);
            hasher.update(sibling);
        } else {
            // If current node is right child, hash(sibling + current)
            hasher.update(sibling);
            hasher.update(current_hash);
        }
        current_hash.copy_from_slice(&hasher.finalize_reset());

        // Move up to the parent level
        index /= 2;
    }

    current_hash
}

pub fn get_beacon_root_from_state(state: &EthereumState, timestamp: U256) -> B256 {
    let db = TrieDB::new(state, HashMap::default(), HashMap::default());
    let timestamp_idx = timestamp % HISTORY_BUFFER_LENGTH;
    let root_idx = timestamp_idx + HISTORY_BUFFER_LENGTH;

    let root = db.storage_ref(BEACON_ROOTS_ADDRESS, root_idx).unwrap();

    root.into()
}
