use alloy_consensus::{Header, ReceiptEnvelope};
use alloy_eips::{eip2718::Eip2718Error, Decodable2718, Encodable2718};
use alloy_provider::{network::AnyNetwork, Provider};
use alloy_rpc_types::{AnyReceiptEnvelope, Log as RpcLog};

use crate::HostError;

#[derive(Debug, Clone)]
pub struct LogsPrefetcher<P: Provider<AnyNetwork> + Clone> {
    provider: P,
    prefetch: bool,
}

impl<P: Provider<AnyNetwork> + Clone> LogsPrefetcher<P> {
    /// Creates a new [`HostExecutor`] with a specific [`Provider`] and [`BlockNumberOrTag`].
    pub fn new(provider: P) -> Self {
        Self { provider, prefetch: false }
    }

    /// Trigger receipts prefetching.
    pub fn trigger_prefetch(&mut self) {
        self.prefetch = true;
    }

    /// Prefetch receipts for inclusion in [`sp1_cc_client_executor::EVMStateSketch`].
    pub async fn prefetch_receipts(
        &self,
        header: &Header,
    ) -> Result<Vec<ReceiptEnvelope>, HostError> {
        if !self.prefetch {
            return Ok(vec![]);
        }

        self.provider
            .get_block_receipts(header.number.into())
            .await?
            .unwrap_or_default()
            .into_iter()
            .map(|r| convert_receipt_envelope(r.inner.inner))
            .collect::<Result<_, _>>()
            .map_err(Into::into)
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
