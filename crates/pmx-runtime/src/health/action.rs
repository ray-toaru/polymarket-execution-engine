use serde::{Deserialize, Serialize};

use super::RuntimeSignal;
use crate::{HealthLevel, RuntimeWorkerKind};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeWorkerAction {
    pub kind: RuntimeWorkerKind,
    pub capability: String,
    pub should_fail_closed: bool,
    pub should_update_runtime_store: bool,
    pub reason: String,
}

pub fn worker_actions_from_runtime_signals(signals: &[RuntimeSignal]) -> Vec<RuntimeWorkerAction> {
    signals
        .iter()
        .map(|signal| {
            let health = signal.to_capability_health();
            let kind = signal.worker_kind();
            RuntimeWorkerAction {
                kind,
                capability: health.capability.clone(),
                should_fail_closed: health.blocks_submit(),
                should_update_runtime_store: true,
                reason: health
                    .last_error
                    .clone()
                    .unwrap_or_else(|| format!("{:?}", health.level)),
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeWorkerStoreWrite {
    pub account_id: String,
    pub capability: String,
    pub worker_kind: RuntimeWorkerKind,
    pub status: HealthLevel,
    pub should_fail_closed: bool,
    pub reason: String,
}

/// Prepare deterministic store-write payloads for runtime worker observations.
///
/// This helper deliberately does not talk to PostgreSQL. Store crates can map the returned
/// payload into `runtime_worker_observations` or capability-specific truth tables after
/// applying their own transaction and idempotency policy.
pub fn runtime_worker_store_writes(
    account_id: impl Into<String>,
    signals: &[RuntimeSignal],
) -> Vec<RuntimeWorkerStoreWrite> {
    let account_id = account_id.into();
    signals
        .iter()
        .map(|signal| {
            let health = signal.to_capability_health();
            let kind = signal.worker_kind();
            let status = health.level.clone();
            let reason = health
                .last_error
                .clone()
                .unwrap_or_else(|| format!("{status:?}"));
            let should_fail_closed = health.blocks_submit();
            RuntimeWorkerStoreWrite {
                account_id: account_id.clone(),
                capability: health.capability,
                worker_kind: kind,
                status,
                should_fail_closed,
                reason,
            }
        })
        .collect()
}
