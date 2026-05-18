use super::*;
use crate::DEFAULT_RUNTIME_OBSERVATION_TTL_SECONDS;
use chrono::Duration;

#[tokio::test]
async fn runtime_worker_observations_degrade_loaded_runtime_state() {
    let store = InMemoryStore::default();
    store.set_runtime_state_for_test(
        "acct-runtime-observed",
        "cond-runtime-observed",
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
        .record_runtime_worker_observation(&RuntimeWorkerObservation {
            account_id: "acct-runtime-observed".into(),
            capability: "heartbeat-lease".into(),
            worker_kind: "HeartbeatLease".into(),
            status: "STALE".into(),
            should_fail_closed: true,
            reason: "lease expired".into(),
            observed_at: None,
        })
        .await
        .expect("record observation");
    let state = store
        .load_runtime_state(&RuntimeStateQuery {
            account_id: "acct-runtime-observed".into(),
            condition_id: "cond-runtime-observed".into(),
            collateral_profile_id: None,
            required_capabilities: vec!["heartbeat".into()],
        })
        .await
        .expect("load runtime state");
    assert_eq!(state.worker_status, WorkerStatus::Stale);
    assert!(
        state
            .required_capabilities
            .contains(&"heartbeat-lease".into())
    );
}

#[tokio::test]
async fn stale_runtime_worker_observations_are_ignored() {
    let store = InMemoryStore::default();
    store.set_runtime_state_for_test(
        "acct-runtime-stale-observation",
        "cond-runtime-stale-observation",
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
            worker_id: "worker-runtime-stale-observation".into(),
            role: "Heartbeat".into(),
            capability: "heartbeat".into(),
            status: "HEALTHY".into(),
            last_heartbeat_at: Utc::now(),
            last_error: None,
        })
        .await
        .expect("record heartbeat");
    store
        .record_runtime_worker_observation(&RuntimeWorkerObservation {
            account_id: "acct-runtime-stale-observation".into(),
            capability: "heartbeat-lease".into(),
            worker_kind: "HeartbeatLease".into(),
            status: "STALE".into(),
            should_fail_closed: true,
            reason: "old lease expiry".into(),
            observed_at: Some(
                Utc::now() - Duration::seconds(DEFAULT_RUNTIME_OBSERVATION_TTL_SECONDS + 1),
            ),
        })
        .await
        .expect("record stale observation");
    let state = store
        .load_runtime_state(&RuntimeStateQuery {
            account_id: "acct-runtime-stale-observation".into(),
            condition_id: "cond-runtime-stale-observation".into(),
            collateral_profile_id: None,
            required_capabilities: vec!["heartbeat".into()],
        })
        .await
        .expect("load runtime state");
    assert_eq!(state.worker_status, WorkerStatus::Healthy);
    assert!(
        !state
            .required_capabilities
            .contains(&"heartbeat-lease".into())
    );
}
