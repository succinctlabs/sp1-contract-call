use alloy_provider::{network::AnyNetwork, Provider};
use alloy_rpc_types::BlockNumberOrTag;
use alloy_sol_types::SolCall;
use alloy_transport::Transport;
use eyre::{eyre, OptionExt};
use reth_primitives::{Block, Header};
use revm::db::CacheDB;
use revm_primitives::U256;
use rsp_rpc_db::RpcDb;

use sp1_cc_client_executor::{io::EVMStateSketch, new_evm, ContractInput};

/// An executor that fetches data from a [`Provider`].
///
/// This executor keeps track of the state being accessed, and eventually compresses it into an
/// [`EVMStateSketch`].
#[derive(Debug, Clone)]
pub struct HostExecutor<T: Transport + Clone, P: Provider<T, AnyNetwork> + Clone> {
    /// The state root of the block to execute our view functions on.
    pub header: Header,
    /// The [`RpcDb`] used to back the EVM.
    pub rpc_db: RpcDb<T, P>,
}

impl<'a, T: Transport + Clone, P: Provider<T, AnyNetwork> + Clone> HostExecutor<T, P> {
    /// Create a new [`HostExecutor`] with a specific [`Provider`] and [`BlockNumberOrTag`].
    pub async fn new(provider: P, block_number: BlockNumberOrTag) -> eyre::Result<Self> {
        let current_block = provider
            .get_block_by_number(block_number, true)
            .await?
            .map(|block| Block::try_from(block.inner))
            .ok_or(eyre!("couldn't fetch block: {}", block_number))??;

        let rpc_db = RpcDb::new(provider, block_number.into(), current_block.state_root);
        Ok(Self { header: current_block.header, rpc_db })
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
    pub async fn finalize(&self) -> EVMStateSketch {
        let account_proofs = self.rpc_db.fetch_used_accounts_and_proofs().await;
        let block_hashes = self.rpc_db.block_hashes.borrow().clone();

        EVMStateSketch {
            header: self.header.clone(),
            storage_and_account_proofs: account_proofs,
            block_hashes,
        }
    }
}
