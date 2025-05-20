use alloy_eips::BlockId;
use alloy_provider::{network::AnyNetwork, Provider, RootProvider};
use rsp_primitives::genesis::Genesis;
use rsp_rpc_db::RpcDb;
use url::Url;

use crate::{
    anchor_builder::{AnchorBuilder, BeaconAnchorBuilder, HeaderAnchorBuilder},
    EvmSketch, HostError,
};

/// A builder for [`EvmSketch`].
#[derive(Debug)]
pub struct EvmSketchBuilder<P, A> {
    block: BlockId,
    genesis: Genesis,
    provider: P,
    anchor_prefetcher: A,
}

impl<P, A> EvmSketchBuilder<P, A> {
    /// Sets the block on which the contract will be called.
    pub fn at_block<B: Into<BlockId>>(mut self, block: B) -> Self {
        self.block = block.into();
        self
    }
    /// Sets the chain on which the contract will be called.
    pub fn with_genesis(mut self, genesis: Genesis) -> Self {
        self.genesis = genesis;
        self
    }
}

impl EvmSketchBuilder<(), ()> {
    /// Sets the Ethereum HTTP RPC endpoint that will be used.
    pub fn el_rpc_url(
        self,
        rpc_url: Url,
    ) -> EvmSketchBuilder<RootProvider<AnyNetwork>, HeaderAnchorBuilder<RootProvider<AnyNetwork>>>
    {
        let provider = RootProvider::new_http(rpc_url);
        EvmSketchBuilder {
            block: self.block,
            genesis: self.genesis,
            provider: provider.clone(),
            anchor_prefetcher: HeaderAnchorBuilder::new(provider),
        }
    }
}

impl<P> EvmSketchBuilder<P, HeaderAnchorBuilder<P>>
where
    P: Provider<AnyNetwork>,
{
    /// Sets the Beacon HTTP RPC endpoint that will be used.
    pub fn cl_rpc_url(self, rpc_url: Url) -> EvmSketchBuilder<P, BeaconAnchorBuilder<P>> {
        EvmSketchBuilder {
            block: self.block,
            genesis: self.genesis,
            provider: self.provider,
            anchor_prefetcher: BeaconAnchorBuilder::new(self.anchor_prefetcher, rpc_url),
        }
    }
}

impl<P, A> EvmSketchBuilder<P, A>
where
    P: Provider<AnyNetwork> + Clone,
    A: AnchorBuilder,
{
    /// Builds an [`EvmSketch`].
    pub async fn build(self) -> Result<EvmSketch<P>, HostError> {
        let anchor = self.anchor_prefetcher.build(self.block).await?;
        let block_number = anchor.header().number;

        let sketch = EvmSketch {
            genesis: self.genesis,
            anchor,
            rpc_db: RpcDb::new(self.provider.clone(), block_number),
            receipts: None,
            provider: self.provider,
        };

        Ok(sketch)
    }
}

impl Default for EvmSketchBuilder<(), ()> {
    fn default() -> Self {
        Self {
            block: BlockId::default(),
            genesis: Genesis::Mainnet,
            provider: (),
            anchor_prefetcher: (),
        }
    }
}
