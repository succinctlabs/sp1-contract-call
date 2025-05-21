use alloy_primitives::B256;
use ethereum_consensus::types::mainnet::SignedBeaconBlock;
use reqwest::Client as ReqwestClient;
use serde::Deserialize;
use url::Url;

use crate::BeaconError;

/// A client used for connecting and querying a beacon node.
#[derive(Debug, Clone)]
pub struct BeaconClient {
    rpc_url: Url,
    client: ReqwestClient,
}

/// The data format returned by official Eth Beacon Node APIs.
#[derive(Debug, Deserialize)]
struct BeaconData<T> {
    #[allow(unused)]
    pub execution_optimistic: bool,
    #[allow(unused)]
    pub finalized: bool,
    pub data: T,
}

impl BeaconClient {
    pub fn new(rpc_url: Url) -> Self {
        Self { rpc_url, client: ReqwestClient::new() }
    }

    /// Gets the block header at the given `beacon_id`.
    pub async fn get_block(&self, beacon_id: String) -> Result<SignedBeaconBlock, BeaconError> {
        let endpoint = format!("{}eth/v2/beacon/blocks/{}", self.rpc_url, beacon_id);

        let response = self.client.get(&endpoint).send().await?;
        let parsed = response.error_for_status()?.json::<BeaconData<SignedBeaconBlock>>().await?;

        Ok(parsed.data)
    }

    /// Retrieves the execution bock hash  at the given `beacon_id`.
    pub async fn get_execution_payload_block_hash(
        &self,
        beacon_id: String,
    ) -> Result<B256, BeaconError> {
        let block = self.get_block(beacon_id).await?;
        let block_hash = match block {
            SignedBeaconBlock::Phase0(_) => None,
            SignedBeaconBlock::Altair(_) => None,
            SignedBeaconBlock::Bellatrix(b) => Some(b.message.body.execution_payload.block_hash),
            SignedBeaconBlock::Capella(b) => Some(b.message.body.execution_payload.block_hash),
            SignedBeaconBlock::Deneb(b) => Some(b.message.body.execution_payload.block_hash),
        };

        block_hash.ok_or_else(|| BeaconError::ExecutionPayloadMissing).map(|h| B256::from_slice(&h))
    }
}
