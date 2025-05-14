use ethereum_consensus::types::mainnet::SignedBeaconBlock;
use reqwest::Client as ReqwestClient;
use serde::Deserialize;
use url::Url;

use crate::HostError;

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
    pub async fn get_block(&self, beacon_id: String) -> Result<SignedBeaconBlock, HostError> {
        let endpoint = format!("{}eth/v2/beacon/blocks/{}", self.rpc_url, beacon_id);

        let response = self.client.get(&endpoint).send().await?;
        let parsed = response.error_for_status()?.json::<BeaconData<SignedBeaconBlock>>().await?;

        Ok(parsed.data)
    }
}
