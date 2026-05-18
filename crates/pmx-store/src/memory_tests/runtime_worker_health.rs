use super::*;
use crate::runtime_observation_ttl_seconds;
use chrono::Duration;
use pmx_core::{CollateralProfileStatus, GeoblockStatus, RuntimeStateSummary, WorkerStatus};

#[tokio::test]
async fn in_memory_worker_heartbeat_informs_runtime_state() {
    let store = InMemoryStore::default();
    store.set_runtime_state_for_test(
        "acct-heartbeat",
        "cond-heartbeat",
        None,
        RuntimeStateSummary {
            geoblock_status: GeoblockStatus::Allowed,
            worker_status: WorkerStatus::Unknown,
            collateral_profile_status: CollateralProfileStatus::DefaultResolved,
            kill_switch_enabled: false,
            required_capabilities: vec!["heartbeat".into()],
        },
    );
    store
        .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
            worker_id: "worker-heartbeat-1".into(),
            role: "Heartbeat".into(),
            capability: "heartbeat".into(),
            status: "HEALTHY".into(),
            last_heartbeat_at: Utc::now(),
            last_error: None,
        })
        .await
        .expect("record heartbeat");
    let state = store
        .load_runtime_state(&RuntimeStateQuery {
            account_id: "acct-heartbeat".into(),
            condition_id: "cond-heartbeat".into(),
            collateral_profile_id: None,
            required_capabilities: vec!["heartbeat".into()],
        })
        .await
        .expect("runtime state");
    assert_eq!(state.worker_status, WorkerStatus::Healthy);
}

#[tokio::test]
async fn stale_in_memory_worker_heartbeat_fails_closed() {
    let store = InMemoryStore::default();
    store.set_runtime_state_for_test(
        "acct-heartbeat-stale",
        "cond-heartbeat-stale",
        None,
        RuntimeStateSummary {
            geoblock_status: GeoblockStatus::Allowed,
            worker_status: WorkerStatus::Healthy,
            collateral_profile_status: CollateralProfileStatus::DefaultResolved,
            kill_switch_enabled: false,
            required_capabilities: vec!["heartbeat".into()],
        },
    );
    store
        .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
            worker_id: "worker-heartbeat-stale".into(),
            role: "Heartbeat".into(),
            capability: "heartbeat".into(),
            status: "HEALTHY".into(),
            last_heartbeat_at: Utc::now()
                - Duration::seconds(runtime_observation_ttl_seconds() + 1),
            last_error: Some("missed heartbeat".into()),
        })
        .await
        .expect("record heartbeat");
    let state = store
        .load_runtime_state(&RuntimeStateQuery {
            account_id: "acct-heartbeat-stale".into(),
            condition_id: "cond-heartbeat-stale".into(),
            collateral_profile_id: None,
            required_capabilities: vec!["heartbeat".into()],
        })
        .await
        .expect("runtime state");
    assert_eq!(state.worker_status, WorkerStatus::Stale);
}

#[tokio::test]
async fn in_memory_lists_runtime_worker_status() {
    let store = InMemoryStore::default();
    let observed_at = Utc::now();
    store
        .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
            worker_id: "worker-status-query".into(),
            role: "Heartbeat".into(),
            capability: "heartbeat".into(),
            status: "HEALTHY".into(),
            last_heartbeat_at: observed_at,
            last_error: None,
        })
        .await
        .expect("record heartbeat");
    store
        .record_runtime_worker_observation(&RuntimeWorkerObservation {
            account_id: "acct-status-query".into(),
            capability: "heartbeat-lease".into(),
            worker_kind: "HeartbeatLease".into(),
            status: "STALE".into(),
            should_fail_closed: true,
            reason: "lease expired".into(),
            observed_at: Some(observed_at),
        })
        .await
        .expect("record observation");
    let report = store
        .list_runtime_worker_status(&RuntimeWorkerStatusQuery {
            account_id: "acct-status-query".into(),
            limit: 100,
            before_observed_at: None,
        })
        .await
        .expect("list runtime worker status");
    assert_eq!(report.heartbeats.len(), 1);
    assert_eq!(report.heartbeats[0].worker_id, "worker-status-query");
    assert_eq!(report.observations.len(), 1);
    assert_eq!(report.observations[0].capability, "heartbeat-lease");
    assert!(report.observations[0].should_fail_closed);
}
