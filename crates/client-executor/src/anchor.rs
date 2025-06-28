use std::collections::HashMap;

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
pub const BLOCK_HASH_LEAF_INDEX: usize = 6444;
/// The generalized Merkle tree index of the `state_root` field in the `BeaconBlock`.
pub const STATE_ROOT_LEAF_INDEX: usize = 6434;

/// Ethereum anchoring system for verifying block execution against beacon chain roots.
///
/// This module provides structures and functionality for creating cryptographic anchors
/// that link Ethereum execution blocks to their corresponding beacon chain state. These
/// anchors enable verification of block validity through Merkle proofs and beacon root
/// commitments stored via EIP-4788.
///
/// # Anchor Types
///
/// - **Header Anchor**: Direct reference to an execution block header
/// - **EIP-4788 Anchor**: Links execution block to beacon chain via EIP-4788 beacon roots
/// - **Chained EIP-4788 Anchor**: Multi-hop verification through state transitions
/// - **Consensus Anchor**: Direct beacon chain consensus verification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Anchor {
    Header(HeaderAnchor),
    Eip4788(BeaconWithHeaderAnchor),
    ChainedEip4788(ChainedBeaconAnchor),
    Consensus(BeaconWithHeaderAnchor),
}

impl Anchor {
    /// Returns the execution block header for this anchor.
    pub fn header(&self) -> &Header {
        match self {
            Anchor::Header(header_anchor) => &header_anchor.header,
            Anchor::Eip4788(beacon_anchor) | Anchor::Consensus(beacon_anchor) => {
                &beacon_anchor.inner.header
            }
            Anchor::ChainedEip4788(chained_anchor) => &chained_anchor.inner.inner.header,
        }
    }

    /// Returns the resolved anchor containing the final identifier and hash after verification.
    pub fn resolve(&self) -> ResolvedAnchor {
        match self {
            Anchor::Header(header_anchor) => ResolvedAnchor {
                id: U256::from(header_anchor.header.number),
                hash: header_anchor.header.hash_slow(),
            },
            Anchor::Eip4788(beacon_anchor) | Anchor::Consensus(beacon_anchor) => {
                let block_hash = beacon_anchor.inner.header.hash_slow();
                let hash = beacon_anchor.anchor.beacon_root(block_hash, BLOCK_HASH_LEAF_INDEX);

                ResolvedAnchor { id: beacon_anchor.id().into(), hash }
            }
            Anchor::ChainedEip4788(chained_anchor) => {
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

    /// Returns the anchor type for this anchor.
    pub fn ty(&self) -> AnchorType {
        match self {
            Anchor::Header(_) => AnchorType::BlockHash,
            Anchor::Eip4788(_) | Anchor::ChainedEip4788(_) => AnchorType::Eip4788,
            Anchor::Consensus(_) => AnchorType::Consensus,
        }
    }
}

impl From<Header> for Anchor {
    fn from(header: Header) -> Self {
        Self::Header(HeaderAnchor { header })
    }
}

/// A resolved anchor containing the final computed identifier and hash.
///
/// This structure represents the result of processing an anchor through its
/// verification chain, yielding a canonical identifier and cryptographic hash
/// that can be used for block validation.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedAnchor {
    pub id: U256,
    pub hash: B256,
}

/// A simple anchor that directly references an Ethereum execution block header.
///
/// This is the most basic form of anchor that provides direct access to a block header
/// without any additional cryptographic proofs or beacon chain integration. It's typically
/// used when you have direct trust in the header data or when working with local/trusted
/// block sources.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeaderAnchor {
    #[serde_as(as = "alloy_consensus::serde_bincode_compat::Header")]
    header: Header,
}

/// An anchor that combines an execution block header with beacon chain verification.
///
/// This structure links an Ethereum execution block to the beacon chain through
/// cryptographic proofs, enabling verification that the execution block is part
/// of the canonical beacon chain. It's used for EIP-4788 based verification
/// where beacon roots are stored in the execution layer.
///
/// The anchor contains:
/// - An execution block header that can be verified
/// - A beacon anchor with Merkle proofs linking the block to beacon chain state
///
/// This enables trustless verification of execution blocks by checking their
/// inclusion in the beacon chain consensus without requiring direct access
/// to beacon chain data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BeaconWithHeaderAnchor {
    inner: HeaderAnchor,
    anchor: BeaconAnchor,
}

impl BeaconWithHeaderAnchor {
    /// Creates a new beacon anchor with header and beacon proof.
    pub fn new(header: Header, anchor: BeaconAnchor) -> Self {
        Self { inner: HeaderAnchor { header }, anchor }
    }

    /// Returns the Merkle proof for beacon chain verification.
    pub fn proof(&self) -> &[B256] {
        self.anchor.proof()
    }

    /// Returns the block identifier used for verification.
    pub fn id(&self) -> &BeaconAnchorId {
        self.anchor.id()
    }

    /// Returns the beacon root for this anchor computed from the execution block hash.
    pub fn beacon_root(&self) -> B256 {
        self.anchor.beacon_root(self.inner.header.hash_slow(), BLOCK_HASH_LEAF_INDEX)
    }
}

impl From<BeaconWithHeaderAnchor> for BeaconAnchor {
    fn from(value: BeaconWithHeaderAnchor) -> Self {
        value.anchor
    }
}

/// A beacon chain anchor that provides cryptographic proof linking data to beacon chain state.
///
/// This structure contains a Merkle proof and identifier that can be used to verify
/// that a specific piece of data (like a block hash or state root) is correctly
/// included in the beacon chain. The proof enables trustless verification without
/// requiring access to the full beacon chain state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BeaconAnchor {
    proof: Vec<B256>,
    id: BeaconAnchorId,
}

impl BeaconAnchor {
    /// Creates a new beacon anchor with the given proof and identifier.
    pub fn new(proof: Vec<B256>, id: BeaconAnchorId) -> Self {
        Self { proof, id }
    }
    /// Creates a new beacon anchor with the given proof and identifier.
    pub fn proof(&self) -> &[B256] {
        &self.proof
    }

    /// Returns the block identifier used for verification.
    pub fn id(&self) -> &BeaconAnchorId {
        &self.id
    }

    /// Reconstructs the beacon chain Merkle root from a leaf value and proof.
    pub fn beacon_root(&self, leaf: B256, generalized_index: usize) -> B256 {
        rebuild_merkle_root(leaf, generalized_index, &self.proof)
    }
}

/// Identifier for a beacon chain anchor, specifying how to locate the anchor in beacon chain
/// history.
///
/// The beacon chain stores historical roots that can be accessed either by timestamp
/// (for EIP-4788 verification) or by slot number (for direct beacon chain verification).
/// This enum allows anchors to specify which indexing method should be used.
///
/// # Variants
///
/// - **Timestamp**: References a beacon root by its timestamp, used with EIP-4788 where beacon
///   roots are stored in the execution layer indexed by timestamp
/// - **Slot**: References a beacon root by its slot number, used for direct beacon chain
///   verification where data is indexed by consensus slots
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BeaconAnchorId {
    Timestamp(u64),
    Slot(u64),
}

impl BeaconAnchorId {
    /// Returns timestamp if this is a Timestamp variant, None otherwise.
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

/// A chained anchor that enables verification through multiple beacon chain state transitions.
///
/// This structure extends the basic beacon anchor concept by allowing verification through
/// a chain of state transitions. It's useful when you need to verify an execution block
/// against a beacon chain state that's not directly accessible via EIP-4788, but can be
/// reached through a series of intermediate states.
///
/// The chained anchor works by:
/// 1. Starting with an execution block and its beacon root (via EIP-4788)
/// 2. Following a chain of beacon chain state transitions through intermediate states
/// 3. Ending at a reference beacon chain state that can be independently verified
///
/// Each step in the chain verifies:
/// - The previous beacon root matches the current state's stored beacon root
/// - The current state root is properly included in the next beacon chain state
///
/// This enables verification of execution blocks against historical beacon chain states
/// that may not be directly accessible, creating a cryptographic chain of trust through
/// intermediate consensus states.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChainedBeaconAnchor {
    inner: BeaconWithHeaderAnchor,
    state_anchors: Vec<BeaconStateAnchor>,
}

impl ChainedBeaconAnchor {
    /// Creates a new chained beacon anchor linking an execution block through multiple state
    /// transitions.
    pub fn new(inner: BeaconWithHeaderAnchor, state_anchors: Vec<BeaconStateAnchor>) -> Self {
        Self { inner, state_anchors }
    }
}

/// An anchor that combines beacon chain state with cryptographic proof for state transition
/// verification.
///
/// This structure represents a single link in a chained verification process, containing
/// both a beacon chain state and the cryptographic proof needed to verify that state's
/// inclusion in the beacon chain. It's used as a building block for `ChainedBeaconAnchor`
/// to enable verification through multiple state transitions.
///
/// The anchor contains:
/// - A complete Ethereum beacon chain state snapshot
/// - A beacon anchor with Merkle proofs linking the state root to beacon chain consensus
///
/// This enables verification that a specific beacon chain state is legitimate and can
/// be used as a trusted intermediate step in a chain of cryptographic proofs. Each
/// `BeaconStateAnchor` in a chain verifies that its state root is properly included
/// in the next beacon chain state, creating a verifiable path from an execution block
/// to a reference beacon chain state.
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

/// Rebuilds a Merkle tree root from a leaf value and its branch proof.
///
/// Given a leaf value, its generalized index in the tree, and the sibling hashes
/// along the path to the root, this function reconstructs the Merkle root by
/// iteratively hashing the current node with its sibling at each level.
///
/// # Arguments
///
/// * `leaf` - The leaf value to start reconstruction from
/// * `generalized_index` - The generalized index of the leaf in the Merkle tree
/// * `branch` - Slice of sibling hashes along the path from leaf to root
///
/// # Returns
///
/// The reconstructed Merkle root hash
pub fn rebuild_merkle_root(leaf: B256, generalized_index: usize, branch: &[B256]) -> B256 {
    let mut current_hash = leaf;
    let depth = generalized_index.ilog2();
    let mut index = generalized_index - (1 << depth);
    let mut hasher = Sha256::new();

    assert_eq!(branch.len() as u32, depth);

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

/// Retrieves a beacon root from Ethereum state using EIP-4788 storage.
///
/// This function looks up a beacon root stored in the EIP-4788 beacon roots contract
/// at the `BEACON_ROOTS_ADDRESS`. The beacon root is indexed by timestamp and retrieved
/// from the circular buffer using modular arithmetic.
///
/// # Arguments
///
/// * `state` - The Ethereum state to query
/// * `timestamp` - The timestamp to look up the beacon root for
///
/// # Returns
///
/// The beacon root hash stored at the given timestamp
pub fn get_beacon_root_from_state(state: &EthereumState, timestamp: U256) -> B256 {
    assert!(!timestamp.is_zero());
    let db = TrieDB::new(state, HashMap::default(), HashMap::default());
    let timestamp_idx = timestamp % HISTORY_BUFFER_LENGTH;
    let root_idx = timestamp_idx + HISTORY_BUFFER_LENGTH;
    let timestamp_in_storage = db.storage_ref(BEACON_ROOTS_ADDRESS, timestamp_idx).unwrap();
    assert_eq!(timestamp, timestamp_in_storage);

    let root = db.storage_ref(BEACON_ROOTS_ADDRESS, root_idx).unwrap();

    root.into()
}
