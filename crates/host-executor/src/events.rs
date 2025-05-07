use alloy_provider::{network::AnyNetwork, Provider};
use alloy_rpc_types::Filter;
use alloy_sol_types::SolEvent;
use sp1_cc_client_executor::{EventsInput, LogsInput};

use crate::HostError;

#[derive(Debug)]
pub struct EventLogsPrefetcher<P: Provider<AnyNetwork> + Clone> {
    provider: P,
}

impl<P: Provider<AnyNetwork> + Clone> EventLogsPrefetcher<P> {
    /// Create a new [`HostExecutor`] with a specific [`Provider`] and [`BlockNumberOrTag`].
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    /// Returns an input containing all logs data and metadata that can be sent to the zkVM.
    ///
    /// If you only need to interact with decoded events, you can use [`prefetch_events`]
    /// That is more cycle efficient.
    pub async fn prefetch_logs(&self, filter: &Filter) -> Result<LogsInput, HostError> {
        let logs = self.provider.get_logs(filter).await?;

        Ok(LogsInput::new(logs))
    }

    /// Returns an input containing only the data needed to decode the event inside the zkVM.
    pub async fn prefetch_events<E: SolEvent>(
        &self,
        filter: &Filter,
    ) -> Result<EventsInput, HostError> {
        let logs = self.provider.get_logs(filter).await?;

        Ok(EventsInput::new(logs))
    }
}
