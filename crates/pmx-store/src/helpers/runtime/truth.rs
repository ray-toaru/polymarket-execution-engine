use async_trait::async_trait;
use pmx_core::{GeoblockStatus, WorkerStatus};

use super::*;
use crate::{
    CanaryRuntimeTruthBindings, CanaryRuntimeTruthQuery, CanaryRuntimeTruthStore,
    RuntimeStateQuery, RuntimeStateStore, RuntimeWorkerHeartbeat, RuntimeWorkerObservation,
    RuntimeWorkerStatusQuery, RuntimeWorkerStatusStore, StoreError,
};

const LIVE_SUBMIT_GATE_CAPABILITY: &str = "live-submit-gate";
const IDEMPOTENCY_LEASE_CAPABILITY: &str = "idempotency-lease";
const ORDER_CANCEL_RECONCILIATION_CAPABILITY: &str = "order-cancel-reconciliation";
const REPOSITORY_RESERVATION_CAPABILITY: &str = "repository-reservation";
const RECONCILE_WORKER_CAPABILITY: &str = "reconcile-worker";
const CANCEL_ONLY_FALLBACK_CAPABILITY: &str = "cancel-only-fallback";
const BALANCE_ALLOWANCE_CHECK_CAPABILITY: &str = "balance-allowance-check";

#[async_trait]
impl<T> CanaryRuntimeTruthStore for T
where
    T: RuntimeStateStore + RuntimeWorkerStatusStore + Send + Sync,
{
    async fn load_canary_runtime_truth(
        &self,
        query: &CanaryRuntimeTruthQuery,
    ) -> Result<CanaryRuntimeTruthBindings, StoreError> {
        let runtime_state = self
            .load_runtime_state(&RuntimeStateQuery {
                account_id: query.account_id.clone(),
                condition_id: query.condition_id.clone(),
                collateral_profile_id: query.collateral_profile_id.clone(),
                required_capabilities: vec![],
            })
            .await?;
        let status = self
            .list_runtime_worker_status(&RuntimeWorkerStatusQuery {
                account_id: query.account_id.clone(),
                limit: 500,
                before_observed_at: None,
            })
            .await?;

        let kill_switch_open = !runtime_state.kill_switch_enabled;
        let runtime_worker_healthy = matches!(runtime_state.worker_status, WorkerStatus::Healthy);
        let geoblock_allowed = matches!(runtime_state.geoblock_status, GeoblockStatus::Allowed);
        let live_submit_gate_ready = capability_ready(
            &status.heartbeats,
            &status.observations,
            LIVE_SUBMIT_GATE_CAPABILITY,
        );
        let idempotency_lease_ready = capability_ready(
            &status.heartbeats,
            &status.observations,
            IDEMPOTENCY_LEASE_CAPABILITY,
        );
        let order_cancel_reconciliation_ready = capability_ready(
            &status.heartbeats,
            &status.observations,
            ORDER_CANCEL_RECONCILIATION_CAPABILITY,
        );
        let repository_reservation_exists = capability_ready(
            &status.heartbeats,
            &status.observations,
            REPOSITORY_RESERVATION_CAPABILITY,
        );
        let reconcile_worker_healthy = capability_ready(
            &status.heartbeats,
            &status.observations,
            RECONCILE_WORKER_CAPABILITY,
        );
        let cancel_only_fallback_ready = capability_ready(
            &status.heartbeats,
            &status.observations,
            CANCEL_ONLY_FALLBACK_CAPABILITY,
        );
        let balance_allowance_checked = capability_ready(
            &status.heartbeats,
            &status.observations,
            BALANCE_ALLOWANCE_CHECK_CAPABILITY,
        );

        let mut evidence_refs = Vec::new();
        if kill_switch_open {
            evidence_refs.push("runtime-state://kill-switch".into());
        }
        for (capability, ready) in [
            (LIVE_SUBMIT_GATE_CAPABILITY, live_submit_gate_ready),
            (IDEMPOTENCY_LEASE_CAPABILITY, idempotency_lease_ready),
            (
                ORDER_CANCEL_RECONCILIATION_CAPABILITY,
                order_cancel_reconciliation_ready,
            ),
            (
                REPOSITORY_RESERVATION_CAPABILITY,
                repository_reservation_exists,
            ),
            (RECONCILE_WORKER_CAPABILITY, reconcile_worker_healthy),
            (CANCEL_ONLY_FALLBACK_CAPABILITY, cancel_only_fallback_ready),
            (
                BALANCE_ALLOWANCE_CHECK_CAPABILITY,
                balance_allowance_checked,
            ),
        ] {
            if ready {
                evidence_refs.push(format!("runtime-state://worker/{capability}"));
            }
        }

        Ok(CanaryRuntimeTruthBindings {
            kill_switch_open,
            live_submit_gate_ready,
            idempotency_lease_ready,
            order_cancel_reconciliation_ready,
            runtime_worker_healthy: Some(runtime_worker_healthy),
            geoblock_allowed: Some(geoblock_allowed),
            repository_reservation_exists: Some(repository_reservation_exists),
            idempotency_key_written: Some(idempotency_lease_ready),
            reconcile_worker_healthy: Some(reconcile_worker_healthy),
            cancel_only_fallback_ready: Some(cancel_only_fallback_ready),
            balance_allowance_checked: Some(balance_allowance_checked),
            evidence_refs,
        })
    }
}

fn capability_ready(
    heartbeats: &[RuntimeWorkerHeartbeat],
    observations: &[RuntimeWorkerObservation],
    capability: &str,
) -> bool {
    let blocked_by_observation = observations
        .iter()
        .filter(|observation| observation.capability == capability)
        .filter(|observation| runtime_observation_is_fresh(observation))
        .any(|observation| {
            observation.should_fail_closed
                || matches!(
                    observation.status.trim().to_ascii_uppercase().as_str(),
                    "STALE" | "ERROR" | "DOWN" | "BLOCKED" | "UNKNOWN" | "UNOBSERVED"
                )
        });
    if blocked_by_observation {
        return false;
    }

    heartbeats
        .iter()
        .filter(|heartbeat| heartbeat.capability == capability)
        .filter(|heartbeat| heartbeat.role == "CanaryRuntimeTruth")
        .max_by_key(|heartbeat| heartbeat.last_heartbeat_at)
        .map(|heartbeat| {
            runtime_worker_heartbeat_is_fresh(heartbeat)
                && matches!(
                    heartbeat.status.trim().to_ascii_uppercase().as_str(),
                    "HEALTHY" | "OK" | "ALLOWED"
                )
        })
        .unwrap_or(false)
}
