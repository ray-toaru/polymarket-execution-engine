use super::*;
use crate::DEFAULT_RUNTIME_OBSERVATION_TTL_SECONDS;
use chrono::Duration;
use pmx_core::AccountId;

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

#[tokio::test]
async fn global_kill_switch_overrides_account_runtime_state() {
    let store = InMemoryStore::default();
    store.set_runtime_state_for_test(
        "acct-runtime-global-kill",
        "cond-runtime-global-kill",
        None,
        RuntimeStateSummary {
            geoblock_status: GeoblockStatus::Allowed,
            worker_status: WorkerStatus::Healthy,
            collateral_profile_status: CollateralProfileStatus::DefaultResolved,
            kill_switch_enabled: false,
            required_capabilities: vec!["heartbeat".into()],
        },
    );
    let query = RuntimeStateQuery {
        account_id: "acct-runtime-global-kill".into(),
        condition_id: "cond-runtime-global-kill".into(),
        collateral_profile_id: None,
        required_capabilities: vec!["heartbeat".into()],
    };
    store
        .set_global_kill_switch(true, "test global block")
        .await
        .expect("set global kill switch");
    let state = store
        .load_runtime_state(&query)
        .await
        .expect("load runtime state");
    assert!(state.kill_switch_enabled);

    store
        .set_global_kill_switch(false, "clear global block")
        .await
        .expect("clear global kill switch");
    let state = store
        .load_runtime_state(&query)
        .await
        .expect("load runtime state");
    assert!(!state.kill_switch_enabled);

    store
        .set_account_kill_switch(
            &AccountId("acct-runtime-global-kill".into()),
            true,
            "account block",
        )
        .await
        .expect("set account kill switch");
    let state = store
        .load_runtime_state(&query)
        .await
        .expect("load runtime state");
    assert!(state.kill_switch_enabled);
}

#[tokio::test]
async fn canary_runtime_truth_requires_store_backed_dependencies() {
    let store = InMemoryStore::default();
    store.set_runtime_state_for_test(
        "acct-canary-truth-partial",
        "cond-canary-truth-partial",
        None,
        RuntimeStateSummary {
            geoblock_status: GeoblockStatus::Allowed,
            worker_status: WorkerStatus::Healthy,
            collateral_profile_status: CollateralProfileStatus::DefaultResolved,
            kill_switch_enabled: false,
            required_capabilities: vec![],
        },
    );
    store
        .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
            worker_id: "worker-live-submit-gate".into(),
            role: "CanaryRuntimeTruth".into(),
            capability: "live-submit-gate".into(),
            status: "HEALTHY".into(),
            last_heartbeat_at: Utc::now(),
            last_error: None,
        })
        .await
        .expect("record live-submit gate");

    let truth = store
        .load_canary_runtime_truth(&CanaryRuntimeTruthQuery {
            account_id: "acct-canary-truth-partial".into(),
            condition_id: "cond-canary-truth-partial".into(),
            collateral_profile_id: None,
        })
        .await
        .expect("load canary runtime truth");
    assert!(truth.kill_switch_open);
    assert!(truth.live_submit_gate_ready);
    assert!(!truth.idempotency_lease_ready);
    assert!(!truth.order_cancel_reconciliation_ready);
    assert_eq!(truth.runtime_worker_healthy, Some(true));
    assert_eq!(truth.geoblock_allowed, Some(true));
    assert_eq!(truth.repository_reservation_exists, Some(false));
    assert_eq!(truth.idempotency_key_written, Some(false));
    assert_eq!(truth.reconcile_worker_healthy, Some(false));
    assert_eq!(truth.cancel_only_fallback_ready, Some(false));
    assert_eq!(truth.balance_allowance_checked, Some(false));
    assert!(!truth.all_ready());
}

#[tokio::test]
async fn canary_runtime_truth_ignores_unscoped_worker_roles() {
    let store = InMemoryStore::default();
    store.set_runtime_state_for_test(
        "acct-canary-truth-role",
        "cond-canary-truth-role",
        None,
        RuntimeStateSummary {
            geoblock_status: GeoblockStatus::Allowed,
            worker_status: WorkerStatus::Healthy,
            collateral_profile_status: CollateralProfileStatus::DefaultResolved,
            kill_switch_enabled: false,
            required_capabilities: vec![],
        },
    );
    for capability in [
        "live-submit-gate",
        "idempotency-lease",
        "order-cancel-reconciliation",
    ] {
        store
            .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                worker_id: format!("unscoped-worker-{capability}"),
                role: "GenericRuntimeWorker".into(),
                capability: capability.into(),
                status: "HEALTHY".into(),
                last_heartbeat_at: Utc::now(),
                last_error: None,
            })
            .await
            .expect("record generic worker heartbeat");
    }

    let truth = store
        .load_canary_runtime_truth(&CanaryRuntimeTruthQuery {
            account_id: "acct-canary-truth-role".into(),
            condition_id: "cond-canary-truth-role".into(),
            collateral_profile_id: None,
        })
        .await
        .expect("load canary runtime truth");
    assert!(truth.kill_switch_open);
    assert!(!truth.live_submit_gate_ready);
    assert!(!truth.idempotency_lease_ready);
    assert!(!truth.order_cancel_reconciliation_ready);
    assert_eq!(truth.runtime_worker_healthy, Some(true));
    assert_eq!(truth.geoblock_allowed, Some(true));
    assert!(!truth.all_ready());
}

#[tokio::test]
async fn canary_runtime_truth_is_ready_when_all_store_dependencies_are_ready() {
    let store = InMemoryStore::default();
    store.set_runtime_state_for_test(
        "acct-canary-truth-ready",
        "cond-canary-truth-ready",
        None,
        RuntimeStateSummary {
            geoblock_status: GeoblockStatus::Allowed,
            worker_status: WorkerStatus::Healthy,
            collateral_profile_status: CollateralProfileStatus::DefaultResolved,
            kill_switch_enabled: false,
            required_capabilities: vec![],
        },
    );
    for capability in [
        "live-submit-gate",
        "idempotency-lease",
        "order-cancel-reconciliation",
        "repository-reservation",
        "reconcile-worker",
        "cancel-only-fallback",
        "balance-allowance-check",
    ] {
        store
            .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                worker_id: format!("worker-{capability}"),
                role: "CanaryRuntimeTruth".into(),
                capability: capability.into(),
                status: "HEALTHY".into(),
                last_heartbeat_at: Utc::now(),
                last_error: None,
            })
            .await
            .expect("record canary runtime truth heartbeat");
    }

    let truth = store
        .load_canary_runtime_truth(&CanaryRuntimeTruthQuery {
            account_id: "acct-canary-truth-ready".into(),
            condition_id: "cond-canary-truth-ready".into(),
            collateral_profile_id: None,
        })
        .await
        .expect("load canary runtime truth");
    assert!(truth.all_ready());
    assert_eq!(truth.runtime_worker_healthy, Some(true));
    assert_eq!(truth.geoblock_allowed, Some(true));
    assert_eq!(truth.repository_reservation_exists, Some(true));
    assert_eq!(truth.idempotency_key_written, Some(true));
    assert_eq!(truth.reconcile_worker_healthy, Some(true));
    assert_eq!(truth.cancel_only_fallback_ready, Some(true));
    assert_eq!(truth.balance_allowance_checked, Some(true));
    assert_eq!(
        truth.evidence_refs,
        vec![
            "runtime-state://kill-switch".to_string(),
            "runtime-state://worker/live-submit-gate".to_string(),
            "runtime-state://worker/idempotency-lease".to_string(),
            "runtime-state://worker/order-cancel-reconciliation".to_string(),
            "runtime-state://worker/repository-reservation".to_string(),
            "runtime-state://worker/reconcile-worker".to_string(),
            "runtime-state://worker/cancel-only-fallback".to_string(),
            "runtime-state://worker/balance-allowance-check".to_string(),
        ]
    );
}
