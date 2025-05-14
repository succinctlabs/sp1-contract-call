use std::fmt::Debug;

use alloy_consensus::{Header, Sealed};
use alloy_eips::BlockId;
use alloy_primitives::B256;
use alloy_provider::{network::AnyNetwork, Provider};
use async_trait::async_trait;
use ethereum_consensus::{ssz::prelude::Prove, types::SignedBeaconBlock};
use sp1_cc_client_executor::{rebuild_merkle_root, Anchor, BeaconAnchor, BLOCK_HASH_LEAF_INDEX};
use url::Url;

use crate::{beacon_client::BeaconClient, HostError};

#[async_trait]
pub trait AnchorBuilder {
    async fn build<B: Into<BlockId> + Send>(&self, block_id: B) -> Result<Anchor, HostError>;
}

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
    pub async fn get_header<B: Into<BlockId>>(&self, block_id: B) -> Result<Header, HostError> {
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

        Ok(header)
    }
}

#[async_trait]
impl<P: Provider<AnyNetwork>> AnchorBuilder for HeaderAnchorBuilder<P> {
    async fn build<B: Into<BlockId> + Send>(&self, block_id: B) -> Result<Anchor, HostError> {
        let header = self.get_header(block_id).await?;

        Ok(header.into())
    }
}

pub struct BeaconAnchorBuilder<P> {
    header_anchor_builder: HeaderAnchorBuilder<P>,
    client: BeaconClient,
}

impl<P> BeaconAnchorBuilder<P> {
    pub fn new(header_anchor_builder: HeaderAnchorBuilder<P>, cl_rpc_url: Url) -> Self {
        Self { header_anchor_builder, client: BeaconClient::new(cl_rpc_url) }
    }
}

#[async_trait]
impl<P: Provider<AnyNetwork>> AnchorBuilder for BeaconAnchorBuilder<P> {
    async fn build<B: Into<BlockId> + Send>(&self, block_id: B) -> Result<Anchor, HostError> {
        let header = self.header_anchor_builder.get_header(block_id).await?;
        let child_header = self.header_anchor_builder.get_header(header.number + 1).await?;
        let header = Sealed::new(header);
        assert_eq!(child_header.parent_hash, header.seal());

        let beacon_root = child_header
            .parent_beacon_block_root
            .ok_or_else(|| HostError::ParentBeaconBlockRootMissing)?;
        let signed_beacon_block = self.client.get_block(beacon_root.to_string()).await?;

        let (proof, _) = match signed_beacon_block {
            SignedBeaconBlock::Deneb(signed_beacon_block) => signed_beacon_block
                .message
                .prove(&["body".into(), "execution_payload".into(), "block_hash".into()])?,
            _ => todo!(),
        };

        assert!(proof.index == BLOCK_HASH_LEAF_INDEX, "the field leaf index is incorrect");

        let proof = proof.branch.iter().map(|n| n.0.into()).collect::<Vec<_>>();

        assert!(
            verify_merkle_root(header.seal(), &proof, beacon_root),
            "the proof verification fail"
        );

        println!("IN HOST: {beacon_root:?}");

        let anchor = BeaconAnchor::new(header.unseal(), proof, child_header.timestamp);

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

fn verify_merkle_root(block_hash: B256, proof: &[B256], beacon_root: B256) -> bool {
    rebuild_merkle_root(block_hash, BLOCK_HASH_LEAF_INDEX, proof) == beacon_root
}
