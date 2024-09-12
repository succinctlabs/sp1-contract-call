use std::collections::BTreeSet;

use alloy_provider::{network::AnyNetwork, Provider};
use alloy_rpc_types::BlockNumberOrTag;
use alloy_sol_types::SolCall;
use alloy_transport::Transport;
use eyre::{eyre, OptionExt};
// use mpt::generate_tries;
use reth_primitives::{Block, Header};
use revm::db::CacheDB;
use revm_primitives::{B256, U256};
use rsp_mpt::EthereumState;
use rsp_primitives::account_proof::eip1186_proof_to_account_proof;
use rsp_rpc_db::RpcDb;

use sp1_cc_client_executor::{io::EVMStateSketch, new_evm, ContractInput};

/// An executor that fetches data from a [`Provider`].
///
/// This executor keeps track of the state being accessed, and eventually compresses it into an
/// [`EVMStateSketch`].
#[derive(Debug, Clone)]
pub struct HostExecutor<T: Transport + Clone, P: Provider<T, AnyNetwork> + Clone> {
    /// The header of the block to execute our view functions on.
    pub header: Header,
    /// The [`RpcDb`] used to back the EVM.
    pub rpc_db: RpcDb<T, P>,
    /// The provider we use to fetch data.
    pub provider: P,
}

impl<'a, T: Transport + Clone, P: Provider<T, AnyNetwork> + Clone> HostExecutor<T, P> {
    /// Create a new [`HostExecutor`] with a specific [`Provider`] and [`BlockNumberOrTag`].
    pub async fn new(provider: P, block_number: BlockNumberOrTag) -> eyre::Result<Self> {
        let block = provider
            .get_block_by_number(block_number, true)
            .await?
            .map(|block| Block::try_from(block.inner))
            .ok_or(eyre!("couldn't fetch block: {}", block_number))??;

        // TODO
        let rpc_db = RpcDb::new(provider.clone(), block_number.as_number().unwrap());
        Ok(Self { header: block.header, rpc_db, provider })
    }

    /// Executes the smart contract call with the given [`ContractInput`].
    pub async fn execute<C: SolCall>(&mut self, call: ContractInput<C>) -> eyre::Result<C::Return> {
        let cache_db = CacheDB::new(&self.rpc_db);
        let mut evm = new_evm(cache_db, &self.header, U256::ZERO, call);
        let output = evm.transact()?;
        let output_bytes = output.result.output().ok_or_eyre("Error getting result")?;

        let result = C::abi_decode_returns(output_bytes, true)?;
        tracing::info!("Result of host executor call: {:?}", output_bytes);
        Ok(result)
    }

    /// Returns the cumulative [`EVMStateSketch`] after executing some smart contracts.
    pub async fn finalize(&self) -> eyre::Result<EVMStateSketch> {
        let block_number = self.header.number;

        // For every account we touched, fetch the storage proofs for all the slots we touched.
        let state_requests = self.rpc_db.get_state_requests();
        tracing::info!("fetching storage proofs");
        let mut storage_proofs = Vec::new();

        for (address, used_keys) in state_requests.iter() {
            let keys = used_keys
                .iter()
                .map(|key| B256::from(*key))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();

            let storage_proof = self
                .provider
                .get_proof(*address, keys.clone())
                .block_id(block_number.into())
                .await?;
            storage_proofs.push(eip1186_proof_to_account_proof(storage_proof));
        }

        let storage_proofs_by_address =
            storage_proofs.iter().map(|item| (item.address, item.clone())).collect();
        let state = EthereumState::from_proofs(self.header.state_root, &storage_proofs_by_address)?;

        // Fetch the parent headers needed to constrain the BLOCKHASH opcode.
        let oldest_ancestor = *self.rpc_db.oldest_ancestor.borrow();
        let mut ancestor_headers = vec![];
        tracing::info!("fetching {} ancestor headers", block_number - oldest_ancestor);
        for height in (oldest_ancestor..=(block_number - 1)).rev() {
            let block = self.provider.get_block_by_number(height.into(), false).await?.unwrap();
            ancestor_headers.push(block.inner.header.try_into()?);
        }

        Ok(EVMStateSketch {
            header: self.header.clone(),
            ancestor_headers,
            state,
            state_requests,
            bytecodes: self.rpc_db.get_bytecodes(),
        })
    }
}
