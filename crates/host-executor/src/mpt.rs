use alloy_primitives::B256;
use reth_trie::AccountProof;
use revm::primitives::HashMap;

use reth_primitives::Address;
use rsp_mpt::mpt::{
    keccak, mpt_from_proof, parse_proof, resolve_nodes, MptNode, MptNodeData, EMPTY_ROOT,
};

use super::EthereumState;

/// Creates a new MPT node from a digest.
fn node_from_digest(digest: B256) -> MptNode {
    match digest {
        EMPTY_ROOT | B256::ZERO => MptNode::default(),
        _ => MptNodeData::Digest(digest).into(),
    }
}

pub fn generate_tries(
    state_root: B256,
    proofs: &HashMap<Address, AccountProof>,
) -> eyre::Result<EthereumState> {
    // if no addresses are provided, return the trie only consisting of the state root
    if proofs.is_empty() {
        return Ok(EthereumState {
            state_trie: node_from_digest(state_root),
            storage_tries: HashMap::new(),
        });
    }

    let mut storage: HashMap<B256, MptNode> = HashMap::with_capacity(proofs.len());

    let mut state_nodes = HashMap::new();
    let mut state_root_node = MptNode::default();
    for (address, proof) in proofs {
        let proof_nodes = parse_proof(&proof.proof).unwrap();
        mpt_from_proof(&proof_nodes).unwrap();

        // the first node in the proof is the root
        if let Some(node) = proof_nodes.first() {
            state_root_node = node.clone();
        }

        proof_nodes.into_iter().for_each(|node| {
            state_nodes.insert(node.reference(), node);
        });

        // if no slots are provided, return the trie only consisting of the storage root
        let storage_root = proof.storage_root;
        if proof.storage_proofs.is_empty() {
            let storage_root_node = node_from_digest(storage_root);
            storage.insert(B256::from(&keccak(address)), storage_root_node);
            continue;
        }

        let mut storage_nodes = HashMap::new();
        let mut storage_root_node = MptNode::default();
        for storage_proof in &proof.storage_proofs {
            let proof_nodes = parse_proof(&storage_proof.proof).unwrap();
            mpt_from_proof(&proof_nodes).unwrap();

            // the first node in the proof is the root
            if let Some(node) = proof_nodes.first() {
                storage_root_node = node.clone();
            }

            proof_nodes.into_iter().for_each(|node| {
                storage_nodes.insert(node.reference(), node);
            });
        }

        // create the storage trie, from all the relevant nodes
        let storage_trie = resolve_nodes(&storage_root_node, &storage_nodes);
        assert_eq!(storage_trie.hash(), storage_root);

        storage.insert(B256::from(&keccak(address)), storage_trie);
    }
    let state_trie = resolve_nodes(&state_root_node, &state_nodes);
    assert_eq!(state_trie.hash(), state_root);

    Ok(EthereumState { state_trie, storage_tries: storage })
}
