use std::{collections::HashMap, iter::once};

use eyre::Result;
use itertools::Itertools;
use reth_primitives::{revm_primitives::AccountInfo, Address, Header, B256, U256};
use reth_trie_common::TrieAccount;
use revm_primitives::{keccak256, Bytecode};
use rsp_client_executor::io::WitnessInput;
use rsp_mpt::EthereumState;
use rsp_witness_db::WitnessDb;
use serde::{Deserialize, Serialize};

/// Information about how the contract executions accessed state, which is needed to execute the
/// contract in SP1.
///
/// Instead of passing in the entire state, only the state roots and merkle proofs
/// for the storage slots that were modified and accessed are passed in.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EVMStateSketch {
    /// The current block header.
    pub header: Header,
    /// The previous block headers starting from the most recent. These are used for calls to the
    /// blockhash opcode.
    pub ancestor_headers: Vec<Header>,
    /// Current block's Ethereum state.
    pub state: EthereumState,
    /// Requests to account state and storage slots.
    pub state_requests: HashMap<Address, Vec<U256>>,
    /// Account bytecodes.
    pub bytecodes: Vec<Bytecode>,
}

// impl WitnessInput for EVMStateSketch {
//     #[inline(always)]
//     fn state(&self) -> &EthereumState {
//         &self.state
//     }

//     #[inline(always)]
//     fn state_anchor(&self) -> B256 {
//         self.header.state_root
//     }

//     #[inline(always)]
//     fn state_requests(&self) -> impl Iterator<Item = (&Address, &Vec<U256>)> {
//         self.state_requests.iter()
//     }

//     #[inline(always)]
//     fn bytecodes(&self) -> impl Iterator<Item = &Bytecode> {
//         self.bytecodes.iter()
//     }

//     #[inline(always)]
//     fn headers(&self) -> impl Iterator<Item = &Header> {
//         once(&self.header).chain(self.ancestor_headers.iter())
//     }
// }

// impl EVMStateSketch {
//     /// Creates a [`WitnessDb`] from an [`EVMStateSketch`]. To do so, it verifies the used
//     /// storage proofs and constructs the account and storage values.
//     #[inline(always)]
//     pub fn redirect_witness_db(&self) -> Result<WitnessDb> {
//         <EVMStateSketch as WitnessInput>::witness_db(self)
//     }
// }

impl EVMStateSketch {
    /// Creates a [`WitnessDb`] from an [`EVMStateSketch`]. To do so, it verifies the used
    /// storage proofs and constructs the account and storage values.
    ///
    /// Note: This mutates the input and takes ownership of used storage proofs and block hashes
    /// to avoid unnecessary cloning.
    pub fn witness_db(&mut self) -> Result<WitnessDb> {
        let bytecodes_by_hash =
            self.bytecodes.iter().map(|code| (code.hash_slow(), code)).collect::<HashMap<_, _>>();

        let mut accounts = HashMap::new();
        let mut storage = HashMap::new();
        let state_requests = std::mem::take(&mut self.state_requests);
        for (address, slots) in state_requests {
            let hashed_address = keccak256(address);
            let hashed_address = hashed_address.as_slice();

            let account_in_trie = self.state.state_trie.get_rlp::<TrieAccount>(hashed_address)?;

            accounts.insert(
                address,
                match account_in_trie {
                    Some(account_in_trie) => AccountInfo {
                        balance: account_in_trie.balance,
                        nonce: account_in_trie.nonce,
                        code_hash: account_in_trie.code_hash,
                        code: Some(
                            (*bytecodes_by_hash
                                .get(&account_in_trie.code_hash)
                                .ok_or_else(|| eyre::eyre!("missing bytecode"))?)
                            // Cloning here is fine as `Bytes` is cheap to clone.
                            .to_owned(),
                        ),
                    },
                    None => Default::default(),
                },
            );

            if !slots.is_empty() {
                let mut address_storage = HashMap::new();

                let storage_trie = self
                    .state
                    .storage_tries
                    .get(hashed_address)
                    .ok_or_else(|| eyre::eyre!("parent state does not contain storage trie"))?;

                for slot in slots {
                    let slot_value = storage_trie
                        .get_rlp::<U256>(keccak256(slot.to_be_bytes::<32>()).as_slice())?
                        .unwrap_or_default();
                    address_storage.insert(slot, slot_value);
                }

                storage.insert(address, address_storage);
            }
        }

        // Verify and build block hashes
        let mut block_hashes: HashMap<u64, B256> = HashMap::new();
        for (child_header, parent_header) in
            once(&self.header).chain(self.ancestor_headers.iter()).tuple_windows()
        {
            if parent_header.number != child_header.number - 1 {
                eyre::bail!("non-consecutive blocks");
            }

            if parent_header.hash_slow() != child_header.parent_hash {
                eyre::bail!("parent hash mismatch");
            }

            block_hashes.insert(parent_header.number, child_header.parent_hash);
        }

        Ok(WitnessDb { accounts, storage, block_hashes })
    }

    /// Convenience method for fetching the state root from the header.
    pub fn state_root(&self) -> B256 {
        self.header.state_root
    }
}
