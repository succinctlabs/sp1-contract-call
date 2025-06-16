use std::{collections::BTreeSet, marker::PhantomData};

use alloy_consensus::ReceiptEnvelope;
use alloy_eips::{eip2718::Eip2718Error, Decodable2718, Encodable2718};
use alloy_primitives::{Address, Bytes, B256, U256};
use alloy_provider::{network::AnyNetwork, Provider};
use alloy_rpc_types::{AnyReceiptEnvelope, Filter, Log as RpcLog};
use alloy_sol_types::SolCall;
use eyre::eyre;
use reth_primitives::EthPrimitives;
use revm::{context::result::ExecutionResult, database::CacheDB};
use rsp_mpt::EthereumState;
use rsp_primitives::{account_proof::eip1186_proof_to_account_proof, genesis::Genesis};
use rsp_rpc_db::RpcDb;
use sp1_cc_client_executor::{
    hash_genesis,
    io::{EvmSketchInput, Primitives},
    Anchor, ContractInput,
};

use crate::{EvmSketchBuilder, HostError};

/// ['EvmSketch'] is used to prefetch all the data required to execute a block and query logs in the
/// zkVM.
#[derive(Debug)]
pub struct EvmSketch<P, PT> {
    /// The genesis block specification.
    pub genesis: Genesis,
    /// The anchor to execute our view functions on.
    pub anchor: Anchor,
    /// The [`RpcDb`] used to back the EVM.
    pub rpc_db: RpcDb<P, AnyNetwork>,
    /// The receipts used to retrieve event logs.
    pub receipts: Option<Vec<ReceiptEnvelope>>,
    /// The provider used to fetch data.
    pub provider: P,

    pub phantom: PhantomData<PT>,
}

impl EvmSketch<(), EthPrimitives> {
    pub fn builder() -> EvmSketchBuilder<(), EthPrimitives, ()> {
        EvmSketchBuilder::default()
    }
}

impl<P, PT> EvmSketch<P, PT>
where
    P: Provider<AnyNetwork> + Clone,
    PT: Primitives,
{
    /// Executes a smart contract call.
    ///
    /// The accessed accounts and storages are recorded, and included in a [`EvmSketchInput`]
    /// when [`Self::finalize`] is called.
    pub async fn call<C: SolCall>(
        &self,
        contract_address: Address,
        caller_address: Address,
        calldata: C,
    ) -> eyre::Result<C::Return> {
        let cache_db = CacheDB::new(&self.rpc_db);
        let chain_spec = PT::build_spec(&self.genesis)?;
        let input = ContractInput::new_call(contract_address, caller_address, calldata);
        let output = PT::transact(&input, cache_db, self.anchor.header(), U256::ZERO, chain_spec)
            .map_err(|err| eyre!(err))?;

        let output_bytes = match output.result {
            ExecutionResult::Success { output, .. } => Ok(output.data().clone()),
            ExecutionResult::Revert { output, .. } => Ok(output),
            ExecutionResult::Halt { reason, .. } => Err(eyre!("Execution halted: {reason:?}")),
        }?;

        Ok(C::abi_decode_returns(&output_bytes)?)
    }

    /// Executes a smart contract call, using the provided [`ContractInput`].
    pub async fn call_raw(&self, input: &ContractInput) -> eyre::Result<Bytes> {
        let cache_db = CacheDB::new(&self.rpc_db);
        let chain_spec = PT::build_spec(&self.genesis)?;
        let output = PT::transact(input, cache_db, self.anchor.header(), U256::ZERO, chain_spec)
            .map_err(|err| eyre!(err))?;

        let output_bytes = match output.result {
            ExecutionResult::Success { output, .. } => Ok(output.data().clone()),
            ExecutionResult::Revert { output, .. } => Ok(output),
            ExecutionResult::Halt { reason, .. } => Err(eyre!("Execution halted: {reason:?}")),
        }?;

        Ok(output_bytes)
    }

    /// Executes a smart contract creation.
    pub async fn create(&self, caller_address: Address, calldata: Bytes) -> eyre::Result<Bytes> {
        let cache_db = CacheDB::new(&self.rpc_db);
        let chain_spec = PT::build_spec(&self.genesis)?;
        let input = ContractInput::new_create(caller_address, calldata);
        let output = PT::transact(&input, cache_db, self.anchor.header(), U256::ZERO, chain_spec)
            .map_err(|err| eyre!(err))?;

        let output_bytes = match output.result {
            ExecutionResult::Success { output, .. } => Ok(output.data().clone()),
            ExecutionResult::Revert { output, .. } => Ok(output),
            ExecutionResult::Halt { reason, .. } => Err(eyre!("Execution halted: {reason:?}")),
        }?;

        Ok(output_bytes.clone())
    }

    /// Prefetch the logs matching the provided `filter`, allowing them to be retrieved in the
    /// client using [`get_logs`].
    ///
    /// [`get_logs`]: sp1_cc_client_executor::ClientExecutor::get_logs
    pub async fn get_logs(&mut self, filter: &Filter) -> Result<Vec<RpcLog>, HostError> {
        let logs = self.provider.get_logs(filter).await?;

        if !logs.is_empty() && self.receipts.is_none() {
            let receipts = self
                .provider
                .get_block_receipts(self.anchor.header().number.into())
                .await?
                .unwrap_or_default()
                .into_iter()
                .map(|r| convert_receipt_envelope(r.inner.inner))
                .collect::<Result<_, _>>()?;

            self.receipts = Some(receipts);
        }

        Ok(logs)
    }

    /// Returns the cumulative [`EvmSketchInput`] after executing some smart contracts.
    pub async fn finalize(self) -> Result<EvmSketchInput, HostError> {
        let block_number = self.anchor.header().number;

        // For every account touched, fetch the storage proofs for all the slots touched.
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

            let storage_proof =
                self.provider.get_proof(*address, keys).block_id(block_number.into()).await?;
            storage_proofs.push(eip1186_proof_to_account_proof(storage_proof));
        }

        let storage_proofs_by_address =
            storage_proofs.iter().map(|item| (item.address, item.clone())).collect();
        let state = EthereumState::from_proofs(
            self.anchor.header().state_root,
            &storage_proofs_by_address,
        )?;

        // Fetch the parent headers needed to constrain the BLOCKHASH opcode.
        let oldest_ancestor = *self.rpc_db.oldest_ancestor.read().unwrap();
        let mut ancestor_headers = vec![];
        tracing::info!("fetching {} ancestor headers", block_number - oldest_ancestor);
        for height in (oldest_ancestor..=(block_number - 1)).rev() {
            let block = self.provider.get_block_by_number(height.into()).full().await?.unwrap();
            ancestor_headers.push(
                block
                    .inner
                    .header
                    .inner
                    .clone()
                    .try_into_header()
                    .map_err(|h| HostError::HeaderConversionError(h.number))?,
            );
        }

        let genesis_hash = hash_genesis(&self.genesis);

        Ok(EvmSketchInput {
            anchor: self.anchor,
            genesis: self.genesis,
            ancestor_headers,
            state,
            state_requests,
            bytecodes: self.rpc_db.get_bytecodes(),
            receipts: self.receipts,
            genesis_hash,
        })
    }
}

fn convert_receipt_envelope(
    any_receipt_envelope: AnyReceiptEnvelope<RpcLog>,
) -> Result<ReceiptEnvelope, Eip2718Error> {
    let any_receipt_envelope = AnyReceiptEnvelope {
        inner: any_receipt_envelope.inner.map_logs(|l| l.inner),
        r#type: any_receipt_envelope.r#type,
    };

    let mut buf = vec![];

    any_receipt_envelope.encode_2718(&mut buf);

    ReceiptEnvelope::decode_2718(&mut buf.as_slice())
}

#[cfg(test)]
mod tests {
    use reth_primitives::EthPrimitives;
    use sp1_cc_client_executor::io::EvmSketchInput;

    use crate::EvmSketch;

    // Function that requires T to be `Sync`
    fn assert_sync<T: Sync>() {}

    #[test]
    fn test_sync() {
        assert_sync::<EvmSketch<(), EthPrimitives>>();
        assert_sync::<EvmSketchInput>();
    }
}
