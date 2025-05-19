use alloy_primitives::B256;
use ethereum_consensus::{phase0::SignedBeaconBlockHeader, Fork};
use reqwest::Client as ReqwestClient;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use url::Url;

use crate::BeaconError;

use super::SignedBeaconBlock;

/// A client used for connecting and querying a beacon node.
#[derive(Debug, Clone)]
pub struct BeaconClient {
    rpc_url: Url,
    client: ReqwestClient,
}

/// The raw response returned by the Beacon Node APIs.
#[derive(Debug, Serialize, Deserialize)]
struct BeaconResponse<T> {
    data: T,
}

/// The raw response returned by the Beacon Node APIs.
#[derive(Debug, Serialize, Deserialize)]
struct BeaconRawResponse<'a> {
    pub version: Fork,
    pub execution_optimistic: bool,
    pub finalized: bool,
    #[serde(borrow)]
    data: &'a RawValue,
}

/// The response returned by the `get_block_header` API.
#[derive(Debug, Serialize, Deserialize)]
struct BlockHeaderResponse {
    pub root: B256,
    pub canonical: bool,
    pub header: SignedBeaconBlockHeader,
}

impl<'de> serde::Deserialize<'de> for SignedBeaconBlock {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let BeaconRawResponse { version, data, .. } = BeaconRawResponse::deserialize(deserializer)?;
        let data = match version {
            Fork::Phase0 => serde_json::from_str(data.get()).map(SignedBeaconBlock::Phase0),
            Fork::Altair => serde_json::from_str(data.get()).map(SignedBeaconBlock::Altair),
            Fork::Bellatrix => serde_json::from_str(data.get()).map(SignedBeaconBlock::Bellatrix),
            Fork::Capella => serde_json::from_str(data.get()).map(SignedBeaconBlock::Capella),
            Fork::Deneb => serde_json::from_str(data.get()).map(SignedBeaconBlock::Deneb),
            Fork::Electra => serde_json::from_str(data.get()).map(SignedBeaconBlock::Electra),
        }
        .map_err(serde::de::Error::custom)?;

        Ok(data)
    }
}

impl BeaconClient {
    pub fn new(rpc_url: Url) -> Self {
        Self { rpc_url, client: ReqwestClient::new() }
    }

    /// Gets the block at the given `beacon_id`.
    pub async fn get_block(&self, beacon_id: String) -> Result<SignedBeaconBlock, BeaconError> {
        let endpoint = format!("{}eth/v2/beacon/blocks/{}", self.rpc_url, beacon_id);

        let response = self.client.get(&endpoint).send().await?;
        let block = response.error_for_status()?.json::<SignedBeaconBlock>().await?;

        Ok(block)
    }

    /// Gets the block header at the given given parent root.
    pub async fn get_header_from_parent_root(
        &self,
        parent_root: String,
    ) -> Result<(B256, SignedBeaconBlockHeader), BeaconError> {
        let endpoint = format!("{}eth/v1/beacon/headers", self.rpc_url);

        let response =
            self.client.get(&endpoint).query(&[("parent_root", parent_root)]).send().await?;
        let response =
            response.error_for_status()?.json::<BeaconResponse<Vec<BlockHeaderResponse>>>().await?;

        Ok((response.data[0].root, response.data[0].header.clone()))
    }

    /// Retrieves the execution bock hash at the given `beacon_id`.
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
            SignedBeaconBlock::Electra(b) => Some(b.message.body.execution_payload.block_hash),
        };

        block_hash.ok_or_else(|| BeaconError::ExecutionPayloadMissing).map(|h| B256::from_slice(&h))
    }
}
