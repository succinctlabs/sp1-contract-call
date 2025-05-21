use std::fmt::Debug;

use alloy_consensus::{Header, Sealed};
use alloy_eips::{eip4788::BEACON_ROOTS_ADDRESS, BlockId};
use alloy_primitives::{B256, U256};
use alloy_provider::{network::AnyNetwork, Provider};
use async_trait::async_trait;
use ethereum_consensus::{ssz::prelude::Prove, types::SignedBeaconBlock};
use rsp_mpt::EthereumState;
use sp1_cc_client_executor::{
    get_beacon_root_from_state, rebuild_merkle_root, Anchor, BeaconAnchor, BeaconBlockField,
    BeaconStateAnchor, BeaconWithHeaderAnchor, ChainedBeaconAnchor, HISTORY_BUFFER_LENGTH,
};
use url::Url;

use crate::{beacon_client::BeaconClient, HostError};

/// Abstracts [`Anchor`] creation.
#[async_trait]
pub trait AnchorBuilder {
    async fn build<B: Into<BlockId> + Send>(&self, block_id: B) -> Result<Anchor, HostError>;
}

/// A builder for [`HeaderAnchor`].
#[derive(Debug)]
pub struct HeaderAnchorBuilder<P> {
    provider: P,
}

impl<P> HeaderAnchorBuilder<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }
}

impl<P: Provider<AnyNetwork>> HeaderAnchorBuilder<P> {
    pub async fn get_header<B: Into<BlockId>>(
        &self,
        block_id: B,
    ) -> Result<Sealed<Header>, HostError> {
        let block_id = block_id.into();
        let block = self
            .provider
            .get_block(block_id)
            .await?
            .ok_or_else(|| HostError::BlockNotFoundError(block_id))?;

        let header = block
            .header
            .inner
            .clone()
            .try_into_header()
            .map_err(|_| HostError::HeaderConversionError(block.inner.header.number))?;

        Ok(Sealed::new(header))
    }
}

#[async_trait]
impl<P: Provider<AnyNetwork>> AnchorBuilder for HeaderAnchorBuilder<P> {
    async fn build<B: Into<BlockId> + Send>(&self, block_id: B) -> Result<Anchor, HostError> {
        let header = self.get_header(block_id).await?;

        Ok(header.into_inner().into())
    }
}

/// A builder for [`BeaconAnchor`].
pub struct BeaconAnchorBuilder<P> {
    header_anchor_builder: HeaderAnchorBuilder<P>,
    client: BeaconClient,
}

impl<P> BeaconAnchorBuilder<P> {
    pub fn new(header_anchor_builder: HeaderAnchorBuilder<P>, cl_rpc_url: Url) -> Self {
        Self { header_anchor_builder, client: BeaconClient::new(cl_rpc_url) }
    }
}

impl<P: Provider<AnyNetwork>> BeaconAnchorBuilder<P> {
    pub async fn build_beacon_anchor_with_header(
        &self,
        header: &Sealed<Header>,
        field: BeaconBlockField,
    ) -> Result<BeaconWithHeaderAnchor, HostError> {
        let child_header = self.header_anchor_builder.get_header(header.number + 1).await?;
        assert_eq!(child_header.parent_hash, header.seal());

        let beacon_root = child_header
            .parent_beacon_block_root
            .ok_or_else(|| HostError::ParentBeaconBlockRootMissing)?;

        let anchor = self
            .build_beacon_anchor(beacon_root, U256::from(child_header.timestamp), field)
            .await?;

        if matches!(field, BeaconBlockField::BlockHash) {
            assert!(
                verify_merkle_root(header.seal(), anchor.proof(), usize::from(&field), beacon_root),
                "the proof verification fail, field: {field}",
            );
        }

        Ok(BeaconWithHeaderAnchor::new(header.clone_inner(), anchor))
    }

    pub async fn build_beacon_anchor(
        &self,
        beacon_root: B256,
        timestamp: U256,
        field: BeaconBlockField,
    ) -> Result<BeaconAnchor, HostError> {
        let signed_beacon_block = self.client.get_block(beacon_root.to_string()).await?;

        let (proof, _) = match signed_beacon_block {
            SignedBeaconBlock::Deneb(signed_beacon_block) => {
                signed_beacon_block.message.prove(&[
                    "body".into(),
                    "execution_payload".into(),
                    field.to_string().as_str().into(),
                ])?
            }
            _ => todo!(),
        };

        assert!(proof.index == field, "the field leaf index is incorrect");

        let proof = proof.branch.iter().map(|n| n.0.into()).collect::<Vec<_>>();

        let anchor = BeaconAnchor::new(proof, timestamp);

        Ok(anchor)
    }
}

#[async_trait]
impl<P: Provider<AnyNetwork>> AnchorBuilder for BeaconAnchorBuilder<P> {
    async fn build<B: Into<BlockId> + Send>(&self, block_id: B) -> Result<Anchor, HostError> {
        let header = self.header_anchor_builder.get_header(block_id).await?;
        let anchor =
            self.build_beacon_anchor_with_header(&header, BeaconBlockField::BlockHash).await?;

        Ok(Anchor::Beacon(anchor))
    }
}

impl<P: Debug> Debug for BeaconAnchorBuilder<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BeaconAnchorBuilder")
            .field("header_anchor_builder", &self.header_anchor_builder)
            .finish()
    }
}

/// A builder for [`ChainedBeaconAnchor`].
#[derive(Debug)]
pub struct ChainedBeaconAnchorBuilder<P> {
    beacon_anchor_builder: BeaconAnchorBuilder<P>,
    /// The reference is a successor of the execution block.
    reference: BlockId,
}

impl<P> ChainedBeaconAnchorBuilder<P> {
    pub fn new(beacon_anchor_builder: BeaconAnchorBuilder<P>, reference: BlockId) -> Self {
        Self { beacon_anchor_builder, reference }
    }
}

impl<P: Provider<AnyNetwork>> ChainedBeaconAnchorBuilder<P> {
    async fn get_eip_4788_timestamp(
        &self,
        timestamp: U256,
        block_hash: B256,
    ) -> Result<U256, HostError> {
        let timestamp_idx = timestamp % HISTORY_BUFFER_LENGTH;
        let result = self
            .beacon_anchor_builder
            .header_anchor_builder
            .provider
            .get_storage_at(BEACON_ROOTS_ADDRESS, timestamp_idx)
            .block_id(BlockId::Hash(block_hash.into()))
            .await?;

        Ok(result)
    }

    async fn retrieve_state(
        &self,
        timestamp: U256,
        block_hash: B256,
    ) -> Result<EthereumState, HostError> {
        // Compute the indexes of the two storage slots that will be queried
        let timestamp_idx = timestamp % HISTORY_BUFFER_LENGTH;
        let root_idx = timestamp_idx + HISTORY_BUFFER_LENGTH;

        let provider = &self.beacon_anchor_builder.header_anchor_builder.provider;

        let proof = provider
            .get_proof(BEACON_ROOTS_ADDRESS, vec![timestamp_idx.into(), root_idx.into()])
            .block_id(BlockId::Hash(block_hash.into()))
            .await?;

        let state = EthereumState::from_account_proof(proof)?;

        Ok(state)
    }
}

#[async_trait]
impl<P: Provider<AnyNetwork>> AnchorBuilder for ChainedBeaconAnchorBuilder<P> {
    async fn build<B: Into<BlockId> + Send>(&self, block_id: B) -> Result<Anchor, HostError> {
        let execution_header =
            self.beacon_anchor_builder.header_anchor_builder.get_header(block_id).await?;
        let reference_header =
            self.beacon_anchor_builder.header_anchor_builder.get_header(self.reference).await?;
        assert!(
            execution_header.number < reference_header.number,
            "The execution block must be an ancestor of the reference block"
        );

        // Build an anchor for the execution block containing the beacon root we need to verify
        let execution_anchor = self
            .beacon_anchor_builder
            .build_beacon_anchor_with_header(&execution_header, BeaconBlockField::BlockHash)
            .await?;
        // Build an anchor for the reference block
        let mut current_anchor = Some(
            self.beacon_anchor_builder
                .build_beacon_anchor_with_header(&reference_header, BeaconBlockField::StateRoot)
                .await?
                .into(),
        );
        let mut current_state_block_hash = reference_header.seal();
        let mut state_anchors: Vec<BeaconStateAnchor> = vec![];

        // Loop backwards until we reach the execution block beacon root
        loop {
            let timestamp = self
                .get_eip_4788_timestamp(execution_anchor.timestamp(), current_state_block_hash)
                .await?;
            // Prefetch the beacon roots contract call for timestamp
            let state = self.retrieve_state(timestamp, current_state_block_hash).await?;
            let parent_beacon_root = get_beacon_root_from_state(&state, timestamp);

            state_anchors.insert(0, BeaconStateAnchor::new(state, current_anchor.take().unwrap()));

            // Check if we've reached the execution block beacon root
            if timestamp == U256::from(execution_anchor.timestamp()) {
                assert!(
                    parent_beacon_root == execution_anchor.beacon_root(),
                    "failed to validate final beacon anchor"
                );
                break;
            }

            current_state_block_hash = self
                .beacon_anchor_builder
                .client
                .get_execution_payload_block_hash(parent_beacon_root.to_string())
                .await?;

            // Update the current anchor with the new beacon root
            let _ = current_anchor.replace(
                self.beacon_anchor_builder
                    .build_beacon_anchor(parent_beacon_root, timestamp, BeaconBlockField::StateRoot)
                    .await?,
            );
        }

        Ok(Anchor::Chained(ChainedBeaconAnchor::new(execution_anchor, state_anchors)))
    }
}

fn verify_merkle_root(
    block_hash: B256,
    proof: &[B256],
    generalized_index: usize,
    beacon_root: B256,
) -> bool {
    rebuild_merkle_root(block_hash, generalized_index, proof) == beacon_root
}
