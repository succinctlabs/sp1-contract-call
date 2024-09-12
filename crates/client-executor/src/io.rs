use std::{collections::HashMap, hash::RandomState};

use eyre::Result;
use reth_primitives::{revm_primitives::AccountInfo, Address, Header, B256, U256};
use rsp_primitives::account_proof::AccountProofWithBytecode;
use rsp_witness_db::WitnessDb;
use serde::{Deserialize, Serialize};

/// Information about how the contract executions accessed state, which is needed to execute the
/// contract in SP1.
///
/// Instead of passing in the entire state, only the state roots and merkle proofs
/// for the storage slots that were modified and accessed are passed in.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EVMStateSketch {
    pub header: Header,
    pub storage_and_account_proofs: HashMap<Address, AccountProofWithBytecode, RandomState>,
    pub block_hashes: HashMap<u64, B256>,
}

impl EVMStateSketch {
    /// Creates a [WitnessDb] from an [EVMStateSketch]. To do so, it verifies the used storage
    /// proofs and constructs the account and storage values.
    ///
    /// Note: This mutates the input and takes ownership of used storage proofs and block hashes
    /// to avoid unnecessary cloning.
    pub fn witness_db(&mut self) -> Result<WitnessDb> {
        let mut accounts = HashMap::new();
        let mut storage = HashMap::new();
        let storage_and_account_proofs = std::mem::take(&mut self.storage_and_account_proofs);
        for (address, proof) in storage_and_account_proofs {
            // Verify the storage proof.
            proof.verify(self.header.state_root)?;

            // Update the accounts.
            let account_info = match proof.proof.info {
                Some(account_info) => AccountInfo {
                    nonce: account_info.nonce,
                    balance: account_info.balance,
                    code_hash: account_info.bytecode_hash.unwrap(),
                    code: Some(proof.code),
                },
                None => AccountInfo::default(),
            };
            accounts.insert(address, account_info);

            // Update the storage.
            let storage_values: HashMap<U256, U256> = proof
                .proof
                .storage_proofs
                .into_iter()
                .map(|storage_proof| (storage_proof.key.into(), storage_proof.value))
                .collect();
            storage.insert(address, storage_values);
        }
        Ok(WitnessDb {
            accounts,
            storage,
            block_hashes: std::mem::take(&mut self.block_hashes),
            state_root: self.header.state_root,
            trie_nodes: HashMap::new(),
        })
    }

    /// Convenience method for fetching the state root from the header.
    pub fn state_root(&self) -> B256 {
        self.header.state_root
    }
}
