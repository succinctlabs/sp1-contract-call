use alloy_primitives::Log;
use alloy_rpc_types::Log as RpcLog;
use alloy_sol_types::SolEvent;
use serde::{Deserialize, Serialize};

use crate::ClientError;

/// Input to event logs.
#[derive(Debug, Serialize, Deserialize)]
pub struct EventsInput {
    logs: Vec<Log>,
}

impl EventsInput {
    pub fn new(logs: Vec<RpcLog>) -> Self {
        let logs = logs.into_iter().map(|l| l.into_inner()).collect();

        Self { logs }
    }

    /// Returns an iterator over all logs with decoded events.
    pub fn decoded_logs<E: SolEvent>(
        &self,
    ) -> impl Iterator<Item = Result<Log<E>, ClientError>> + use<'_, E> {
        self.logs.iter().map(move |log| E::decode_log(log).map_err(Into::into))
    }

    /// Returns an iterator over all raw logs.
    ///
    /// This function can be useful for filtering logs (on topics or address) before decoding,
    /// thus saving computing cycles.
    pub fn raw_logs(&self) -> impl Iterator<Item = &Log> {
        self.logs.iter()
    }
}
