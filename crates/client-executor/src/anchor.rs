use alloy_consensus::Header;
use alloy_primitives::{B256, U256};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sha2::{Digest, Sha256};

use crate::AnchorType;

/// The generalized Merkle tree index of the `block_hash` field in the `BeaconBlock`.
pub const BLOCK_HASH_LEAF_INDEX: usize = 6444;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Anchor {
    Header(HeaderAnchor),
    Beacon(BeaconAnchor),
}

impl Anchor {
    pub fn header(&self) -> &Header {
        match self {
            Anchor::Header(header_anchor) => &header_anchor.header,
            Anchor::Beacon(beacon_anchor) => &beacon_anchor.inner.header,
        }
    }

    pub fn id(&self) -> U256 {
        match self {
            Anchor::Header(header_anchor) => U256::from(header_anchor.header.number),
            Anchor::Beacon(beacon_anchor) => U256::from(beacon_anchor.timestamp),
        }
    }

    pub fn hash(&self) -> B256 {
        match self {
            Anchor::Header(header_anchor) => header_anchor.header.hash_slow(),
            Anchor::Beacon(beacon_anchor) => {
                let block_hash = beacon_anchor.inner.header.hash_slow();

                rebuild_merkle_root(block_hash, BLOCK_HASH_LEAF_INDEX, &beacon_anchor.proof)
            }
        }
    }

    pub fn ty(&self) -> AnchorType {
        match self {
            Anchor::Header(_) => AnchorType::BlockHash,
            Anchor::Beacon(_) => AnchorType::BeaconRoot,
        }
    }
}

impl From<Header> for Anchor {
    fn from(header: Header) -> Self {
        Self::Header(HeaderAnchor { header })
    }
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeaderAnchor {
    #[serde_as(as = "alloy_consensus::serde_bincode_compat::Header")]
    header: Header,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BeaconAnchor {
    inner: HeaderAnchor,
    proof: Vec<B256>,
    timestamp: u64,
}

impl BeaconAnchor {
    pub fn new(header: Header, proof: Vec<B256>, timestamp: u64) -> Self {
        Self { inner: HeaderAnchor { header }, proof, timestamp }
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
