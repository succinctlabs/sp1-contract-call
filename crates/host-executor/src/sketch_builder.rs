use alloy_eips::BlockId;
use alloy_provider::{network::AnyNetwork, Provider, RootProvider};
use rsp_primitives::genesis::Genesis;
use rsp_rpc_db::RpcDb;
use url::Url;

use crate::{
    anchor_builder::{
        AnchorBuilder, BeaconAnchorBuilder, ChainedBeaconAnchorBuilder, HeaderAnchorBuilder,
    },
    EvmSketch, HostError,
};

/// A builder for [`EvmSketch`].
#[derive(Debug)]
pub struct EvmSketchBuilder<P, A> {
    block: BlockId,
    genesis: Genesis,
    provider: P,
    anchor_builder: A,
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
            anchor_builder: HeaderAnchorBuilder::new(provider),
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
            anchor_builder: BeaconAnchorBuilder::new(self.anchor_builder, rpc_url),
        }
    }
}

impl<P> EvmSketchBuilder<P, BeaconAnchorBuilder<P>>
where
    P: Provider<AnyNetwork>,
{
    /// Sets the Beacon HTTP RPC endpoint that will be used.
    pub fn at_reference_block<B: Into<BlockId>>(
        self,
        block_id: B,
    ) -> EvmSketchBuilder<P, ChainedBeaconAnchorBuilder<P>> {
        EvmSketchBuilder {
            block: self.block,
            genesis: self.genesis,
            provider: self.provider,
            anchor_builder: ChainedBeaconAnchorBuilder::new(self.anchor_builder, block_id.into()),
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
        let anchor = self.anchor_builder.build(self.block).await?;
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
            anchor_builder: (),
        }
    }
}
