use alloy_provider::{network::AnyNetwork, RootProvider};
use revm_primitives::{b256, uint};
use sp1_cc_host_executor::{
    AnchorBuilder, BeaconAnchorBuilder, ChainedBeaconAnchorBuilder, HeaderAnchorBuilder,
};

#[tokio::test]
async fn test_deneb_beacon_anchor() {
    dotenv::dotenv().ok();

    let eth_rpc_url =
        std::env::var("ETH_RPC_URL").unwrap_or_else(|_| panic!("Missing ETH_RPC_URL"));
    let beacon_rpc_url =
        std::env::var("BEACON_RPC_URL").unwrap_or_else(|_| panic!("Missing BEACON_RPC_URL"));
    let provider = RootProvider::<AnyNetwork>::new_http(eth_rpc_url.parse().unwrap());

    let beacon_anchor_builder = BeaconAnchorBuilder::new(
        HeaderAnchorBuilder::new(provider),
        beacon_rpc_url.parse().unwrap(),
    );

    let anchor = beacon_anchor_builder.build(22300000).await.unwrap();
    let resolved = anchor.resolve();

    assert_eq!(resolved.id, uint!(1745028935_U256)); // Timestamp

    assert_eq!(
        resolved.hash,
        b256!("0xc35d26c08f8e7065e874263f6025b625bca6ed4d3af97da932e5c9be74491ac8")
    )
}

#[tokio::test]
async fn test_electra_beacon_anchor() {
    dotenv::dotenv().ok();

    let eth_rpc_url =
        std::env::var("ETH_RPC_URL").unwrap_or_else(|_| panic!("Missing ETH_RPC_URL"));
    let beacon_rpc_url =
        std::env::var("BEACON_RPC_URL").unwrap_or_else(|_| panic!("Missing BEACON_RPC_URL"));
    let provider = RootProvider::<AnyNetwork>::new_http(eth_rpc_url.parse().unwrap());

    let beacon_anchor_builder = BeaconAnchorBuilder::new(
        HeaderAnchorBuilder::new(provider),
        beacon_rpc_url.parse().unwrap(),
    );

    let anchor = beacon_anchor_builder.build(22500000).await.unwrap();
    let resolved = anchor.resolve();

    assert_eq!(resolved.id, uint!(1747451519_U256)); // Timestamp

    assert_eq!(
        resolved.hash,
        b256!("0xd1fa05159386e8ee0ef3a158a4e37a0a807de7d7e1e2d016f364cec3efcb88f9")
    )
}

#[tokio::test]
async fn test_consensus_beacon_anchor() {
    dotenv::dotenv().ok();

    let eth_rpc_url =
        std::env::var("ETH_RPC_URL").unwrap_or_else(|_| panic!("Missing ETH_RPC_URL"));
    let beacon_rpc_url =
        std::env::var("BEACON_RPC_URL").unwrap_or_else(|_| panic!("Missing BEACON_RPC_URL"));
    let provider = RootProvider::<AnyNetwork>::new_http(eth_rpc_url.parse().unwrap());

    let beacon_anchor_builder = BeaconAnchorBuilder::new(
        HeaderAnchorBuilder::new(provider),
        beacon_rpc_url.parse().unwrap(),
    )
    .into_consensus();

    let anchor = beacon_anchor_builder.build(22500000).await.unwrap();
    let resolved = anchor.resolve();

    assert_eq!(resolved.id, uint!(11718957_U256)); // Slot

    assert_eq!(
        resolved.hash,
        b256!("0xd1fa05159386e8ee0ef3a158a4e37a0a807de7d7e1e2d016f364cec3efcb88f9")
    )
}

#[tokio::test]
async fn test_deneb_chained_beacon_anchor() {
    dotenv::dotenv().ok();

    let eth_rpc_url =
        std::env::var("ETH_RPC_URL").unwrap_or_else(|_| panic!("Missing ETH_RPC_URL"));
    let beacon_rpc_url =
        std::env::var("BEACON_RPC_URL").unwrap_or_else(|_| panic!("Missing BEACON_RPC_URL"));
    let provider = RootProvider::<AnyNetwork>::new_http(eth_rpc_url.parse().unwrap());

    let chained_beacon_anchor_builder = ChainedBeaconAnchorBuilder::new(
        BeaconAnchorBuilder::new(
            HeaderAnchorBuilder::new(provider),
            beacon_rpc_url.parse().unwrap(),
        ),
        22350000.into(),
    );

    let anchor = chained_beacon_anchor_builder.build(22300000).await.unwrap();
    let resolved = anchor.resolve();

    assert_eq!(
        resolved.hash,
        b256!("0x4315c94f7adbe9ad88608b111ddc5ba2240f087248415b51d172e3e89229ddb7")
    )
}

#[tokio::test]
async fn test_electra_chained_beacon_anchor() {
    dotenv::dotenv().ok();

    let eth_rpc_url =
        std::env::var("ETH_RPC_URL").unwrap_or_else(|_| panic!("Missing ETH_RPC_URL"));
    let beacon_rpc_url =
        std::env::var("BEACON_RPC_URL").unwrap_or_else(|_| panic!("Missing BEACON_RPC_URL"));
    let provider = RootProvider::<AnyNetwork>::new_http(eth_rpc_url.parse().unwrap());

    let chained_beacon_anchor_builder = ChainedBeaconAnchorBuilder::new(
        BeaconAnchorBuilder::new(
            HeaderAnchorBuilder::new(provider),
            beacon_rpc_url.parse().unwrap(),
        ),
        22510000.into(),
    );

    let anchor = chained_beacon_anchor_builder.build(22450000).await.unwrap();
    let resolved = anchor.resolve();

    assert_eq!(
        resolved.hash,
        b256!("0x62a94f3faf03493b691de25dda6ecd9150af709a817ebe9e3c6b654aa4b54f81")
    )
}
