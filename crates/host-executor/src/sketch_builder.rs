use std::marker::PhantomData;

use alloy_eips::BlockId;
use alloy_provider::{network::AnyNetwork, Provider, RootProvider};
use reth_primitives::EthPrimitives;
use rsp_primitives::genesis::Genesis;
use rsp_rpc_db::RpcDb;
use sp1_cc_client_executor::io::Primitives;
use url::Url;

use crate::{
    anchor_builder::{
        AnchorBuilder, BeaconAnchorBuilder, ChainedBeaconAnchorBuilder, HeaderAnchorBuilder,
    },
    ConsensusBeaconAnchor, Eip4788BeaconAnchor, EvmSketch, HostError,
};

/// A builder for [`EvmSketch`].
#[derive(Debug)]
pub struct EvmSketchBuilder<P, PT, A> {
    block: BlockId,
    genesis: Genesis,
    provider: P,
    anchor_builder: A,
    phantom: PhantomData<PT>,
}

impl<P, PT, A> EvmSketchBuilder<P, PT, A> {
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

impl<PT> EvmSketchBuilder<(), PT, ()> {
    /// Sets the Ethereum HTTP RPC endpoint that will be used.
    pub fn el_rpc_url(
        self,
        rpc_url: Url,
    ) -> EvmSketchBuilder<
        RootProvider<AnyNetwork>,
        EthPrimitives,
        HeaderAnchorBuilder<RootProvider<AnyNetwork>>,
    > {
        let provider = RootProvider::new_http(rpc_url);
        EvmSketchBuilder {
            block: self.block,
            genesis: self.genesis,
            provider: provider.clone(),
            anchor_builder: HeaderAnchorBuilder::new(provider),
            phantom: PhantomData,
        }
    }
}

#[cfg(feature = "optimism")]
impl<P, A> EvmSketchBuilder<P, EthPrimitives, A> {
    /// Configures the [`EvmSketch`] for OP Stack.
    ///
    /// Note: On the client, the executor should be created with [`ClientExecutor::optimism()`]
    ///
    /// [`ClientExecutor::optimism()`]: sp1_cc_client_executor::ClientExecutor::optimism
    pub fn optimism(self) -> EvmSketchBuilder<P, reth_optimism_primitives::OpPrimitives, A> {
        EvmSketchBuilder {
            block: self.block,
            genesis: self.genesis,
            provider: self.provider,
            anchor_builder: self.anchor_builder,
            phantom: PhantomData,
        }
    }
}

impl<P, PT> EvmSketchBuilder<P, PT, HeaderAnchorBuilder<P>>
where
    P: Provider<AnyNetwork>,
{
    /// Sets the Beacon HTTP RPC endpoint that will be used.
    pub fn cl_rpc_url(
        self,
        rpc_url: Url,
    ) -> EvmSketchBuilder<P, PT, BeaconAnchorBuilder<P, Eip4788BeaconAnchor>> {
        EvmSketchBuilder {
            block: self.block,
            genesis: self.genesis,
            provider: self.provider,
            anchor_builder: BeaconAnchorBuilder::new(self.anchor_builder, rpc_url),
            phantom: self.phantom,
        }
    }
}

impl<P, PT> EvmSketchBuilder<P, PT, BeaconAnchorBuilder<P, Eip4788BeaconAnchor>>
where
    P: Provider<AnyNetwork>,
{
    /// Sets the Beacon HTTP RPC endpoint that will be used.
    pub fn at_reference_block<B: Into<BlockId>>(
        self,
        block_id: B,
    ) -> EvmSketchBuilder<P, PT, ChainedBeaconAnchorBuilder<P>> {
        EvmSketchBuilder {
            block: self.block,
            genesis: self.genesis,
            provider: self.provider,
            anchor_builder: ChainedBeaconAnchorBuilder::new(self.anchor_builder, block_id.into()),
            phantom: self.phantom,
        }
    }

    /// Configures the builder to generate an [`Anchor`] containing the slot number associated to
    /// the beacon block root.
    ///
    /// This is useful for verification methods that have direct access to the state of the beacon
    /// chain, such as systems using beacon light clients.
    ///
    /// [`Anchor`]: sp1_cc_client_executor::Anchor
    pub fn consensus(
        self,
    ) -> EvmSketchBuilder<P, PT, BeaconAnchorBuilder<P, ConsensusBeaconAnchor>> {
        EvmSketchBuilder {
            block: self.block,
            genesis: self.genesis,
            provider: self.provider,
            anchor_builder: self.anchor_builder.into_consensus(),
            phantom: self.phantom,
        }
    }
}

impl<P, PT, A> EvmSketchBuilder<P, PT, A>
where
    P: Provider<AnyNetwork> + Clone,
    PT: Primitives,
    A: AnchorBuilder,
{
    /// Builds an [`EvmSketch`].
    pub async fn build(self) -> Result<EvmSketch<P, PT>, HostError> {
        let anchor = self.anchor_builder.build(self.block).await?;
        let block_number = anchor.header().number;

        let sketch = EvmSketch {
            genesis: self.genesis,
            anchor,
            rpc_db: RpcDb::new(self.provider.clone(), block_number),
            receipts: None,
            provider: self.provider,
            phantom: PhantomData,
        };

        Ok(sketch)
    }
}

impl Default for EvmSketchBuilder<(), EthPrimitives, ()> {
    fn default() -> Self {
        Self {
            block: BlockId::default(),
            genesis: Genesis::Mainnet,
            provider: (),
            anchor_builder: (),
            phantom: PhantomData,
        }
    }
}
