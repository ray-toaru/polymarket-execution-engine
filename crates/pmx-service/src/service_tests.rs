use super::*;
use crate::*;
use crate::{StaticRuntimeStateProvider, StoreBackedRuntimeStateProvider};
use chrono::Utc;
use pmx_policy::evaluate_constraints;
use pmx_runtime::{HeartbeatLeaseCandidate, RuntimeSignal};
use pmx_store::{
    InMemoryStore, OrderLifecycleStore, PostgresStore, RuntimeStateQuery, RuntimeStateStore,
    RuntimeWorkerHealthStore, RuntimeWorkerHeartbeat, RuntimeWorkerStatusQuery,
    RuntimeWorkerStatusStore,
};

fn intent() -> TradeIntent {
    TradeIntent {
        client_intent_id: "client-1".into(),
        account_id: AccountId("acct-1".into()),
        market: MarketRef {
            condition_id: ConditionId("cond-1".into()),
            slug: Some("slug".into()),
            is_sports: false,
        },
        token_id: TokenId("token-1".into()),
        side: Side::Buy,
        quantity: QuantityIntent {
            max_notional: Some(DecimalString("1".into())),
            max_shares: None,
        },
        limit_price: DecimalString("0.5".into()),
        time_in_force: TimeInForce::Gtc,
        collateral_profile_id: None,
    }
}

fn allow_runtime_state() -> RuntimeStateSummary {
    RuntimeStateSummary {
        geoblock_status: GeoblockStatus::Allowed,
        worker_status: WorkerStatus::Healthy,
        collateral_profile_status: CollateralProfileStatus::DefaultResolved,
        kill_switch_enabled: false,
        required_capabilities: vec![],
    }
}

fn approval() -> ApprovalReceipt {
    ApprovalReceipt {
        approval_id: "approval-1".into(),
        approved_by: "operator".into(),
        approved_at: Utc::now(),
        approval_hash: HashValue("approval-hash".into()),
    }
}

fn order(order_id: &str, lifecycle_state: OrderLifecycleState) -> OrderLifecycleRecord {
    OrderLifecycleRecord {
        order_id: order_id.into(),
        execution_id: "exec-order-life".into(),
        account_id: "acct-1".into(),
        condition_id: "cond-1".into(),
        token_id: "token-1".into(),
        side: "BUY".into(),
        lifecycle_state,
        remote_order_id: Some(format!("remote-{order_id}")),
        remote_state: Some("OPEN".into()),
        created_at: None,
        updated_at: None,
    }
}

async fn seed_test_plan(store: &InMemoryStore, execution_id: &str, account_id: &str) {
    store
        .save_plan_summary(&ExecutionPlanSummary {
            execution_id: execution_id.into(),
            account_id: AccountId(account_id.into()),
            normalized_intent_id: format!("norm-{execution_id}"),
            snapshot_id: format!("snap-{execution_id}"),
            decision_id: format!("decision-{execution_id}"),
            plan_hash: HashValue(format!("hash-{execution_id}")),
            status: PlanStatus::Ready,
            max_exposure: DecimalString("0".into()),
            explanation: vec!["test plan for sign-only lifecycle FK parity".into()],
        })
        .await
        .expect("seed execution plan");
}

#[path = "service_tests/flow.rs"]
mod flow;

#[path = "service_tests/non_live_order_lifecycle.rs"]
mod non_live_order_lifecycle;

#[path = "service_tests/runtime_worker_basic.rs"]
mod runtime_worker_basic;

#[path = "service_tests/runtime_worker_lease.rs"]
mod runtime_worker_lease;

#[path = "service_tests/sign_only.rs"]
mod sign_only;

#[tokio::test]
async fn service_records_resource_refresh_worker_tick_for_decision_gate() {
    let store = InMemoryStore::default();
    store.set_runtime_state_for_test("acct-1", "cond-1", None, allow_runtime_state());
    for capability in ["heartbeat", "reconcile", "resource-refresh"] {
        store
            .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                worker_id: format!("worker-{capability}"),
                role: "service-test".into(),
                capability: capability.into(),
                status: "HEALTHY".into(),
                last_heartbeat_at: Utc::now(),
                last_error: None,
            })
            .await
            .expect("record worker heartbeat");
    }

    let observed_at = Utc::now();
    let receipt = record_resource_refresh_worker_tick(
        &store,
        ResourceRefreshWorkerTick {
            account_id: "acct-1".into(),
            provider_name: "resource-refresh-worker-test".into(),
            instance_id: "worker-resource-refresh".into(),
            lease_owner_id: "worker-resource-refresh".into(),
            market_websocket_connected: true,
            market_websocket_stale: false,
            user_websocket_connected: true,
            user_websocket_stale: false,
            geoblock_status: GeoblockStatus::Allowed,
            remote_unknown_orders: 0,
            observed_at,
            stale_after_seconds: 30,
            no_trading_side_effect: true,
            observations: vec![
                pmx_runtime::ResourceRefreshObservation {
                    component: pmx_runtime::ResourceRefreshComponent::Account,
                    resource_id: "acct-1".into(),
                    refreshed_at: observed_at - chrono::Duration::seconds(60),
                    status: pmx_runtime::HealthLevel::Healthy,
                    last_error: None,
                },
                pmx_runtime::ResourceRefreshObservation {
                    component: pmx_runtime::ResourceRefreshComponent::Market,
                    resource_id: "cond-1".into(),
                    refreshed_at: observed_at - chrono::Duration::seconds(5),
                    status: pmx_runtime::HealthLevel::Healthy,
                    last_error: None,
                },
            ],
        },
    )
    .await
    .expect("record resource refresh worker tick");
    assert!(!receipt.evaluation.fresh);
    assert_eq!(receipt.evaluation.stale_components, vec!["account:acct-1"]);
    assert!(receipt.provider_tick.lease_owner_active);
    assert!(!receipt.provider_tick.submit_allowed_by_runtime);

    let service = ExecutorService::with_runtime_provider(
        store.clone(),
        StoreBackedRuntimeStateProvider::new(store.clone()),
        "test-executor".into(),
        DEFAULT_CONTRACT_VERSION.into(),
    );
    let normalized = service.normalize(intent()).await.expect("normalize");
    let snapshot = service
        .capture_snapshot(normalized.clone())
        .await
        .expect("snapshot");
    assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Stale);
    assert!(
        snapshot
            .runtime_state
            .required_capabilities
            .contains(&"resource-refresh".to_string())
    );
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
        })
        .await
        .expect("decision");
    assert_eq!(decision.status, DecisionStatus::Block);
    assert!(decision.reasons.contains(&BlockReason::WorkerStale));
}

#[tokio::test]
async fn service_records_reconcile_backlog_worker_tick_for_decision_gate() {
    let store = InMemoryStore::default();
    store.set_runtime_state_for_test("acct-1", "cond-1", None, allow_runtime_state());
    for capability in ["heartbeat", "reconcile", "resource-refresh"] {
        store
            .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                worker_id: format!("worker-{capability}"),
                role: "service-test".into(),
                capability: capability.into(),
                status: "HEALTHY".into(),
                last_heartbeat_at: Utc::now(),
                last_error: None,
            })
            .await
            .expect("record worker heartbeat");
    }

    let observed_at = Utc::now();
    let receipt = record_reconcile_backlog_worker_tick(
        &store,
        ReconcileBacklogWorkerTick {
            account_id: "acct-1".into(),
            provider_name: "reconcile-backlog-worker-test".into(),
            instance_id: "worker-reconcile-backlog".into(),
            lease_owner_id: "worker-reconcile-backlog".into(),
            market_websocket_connected: true,
            market_websocket_stale: false,
            user_websocket_connected: true,
            user_websocket_stale: false,
            geoblock_status: GeoblockStatus::Allowed,
            resource_refresh_fresh: true,
            remote_unknown_order_ids: vec!["order-remote-unknown".into()],
            observed_at,
            no_trading_side_effect: true,
        },
    )
    .await
    .expect("record reconcile backlog worker tick");
    assert_eq!(receipt.evaluation.remote_unknown_orders, 1);
    assert!(receipt.evaluation.submit_blocked);
    assert!(receipt.provider_tick.lease_owner_active);
    assert!(!receipt.provider_tick.submit_allowed_by_runtime);

    let service = ExecutorService::with_runtime_provider(
        store.clone(),
        StoreBackedRuntimeStateProvider::new(store.clone()),
        "test-executor".into(),
        DEFAULT_CONTRACT_VERSION.into(),
    );
    let normalized = service.normalize(intent()).await.expect("normalize");
    let snapshot = service
        .capture_snapshot(normalized.clone())
        .await
        .expect("snapshot");
    assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Degraded);
    assert!(
        snapshot
            .runtime_state
            .required_capabilities
            .contains(&"reconcile-backlog".to_string())
    );
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
        })
        .await
        .expect("decision");
    assert_eq!(decision.status, DecisionStatus::Block);
    assert!(decision.reasons.contains(&BlockReason::WorkerDegraded));
}

#[tokio::test]
async fn service_records_reconcile_backlog_from_order_lifecycle() {
    let store = InMemoryStore::default();
    store.set_runtime_state_for_test("acct-1", "cond-1", None, allow_runtime_state());
    for capability in ["heartbeat", "reconcile", "resource-refresh"] {
        store
            .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                worker_id: format!("worker-{capability}"),
                role: "service-test".into(),
                capability: capability.into(),
                status: "HEALTHY".into(),
                last_heartbeat_at: Utc::now(),
                last_error: None,
            })
            .await
            .expect("record worker heartbeat");
    }
    store
        .upsert_order_lifecycle(&order(
            "order-lifecycle-backlog",
            OrderLifecycleState::RemoteUnknown,
        ))
        .await
        .expect("upsert remote unknown order");
    store
        .upsert_order_lifecycle(&order(
            "order-lifecycle-posted",
            OrderLifecycleState::Posted,
        ))
        .await
        .expect("upsert posted order");

    let observed_at = Utc::now();
    let receipt = record_reconcile_backlog_from_order_lifecycle(
        &store,
        ReconcileBacklogWorkerTick {
            account_id: "acct-1".into(),
            provider_name: "reconcile-lifecycle-reader-test".into(),
            instance_id: "worker-reconcile-lifecycle-reader".into(),
            lease_owner_id: "worker-reconcile-lifecycle-reader".into(),
            market_websocket_connected: true,
            market_websocket_stale: false,
            user_websocket_connected: true,
            user_websocket_stale: false,
            geoblock_status: GeoblockStatus::Allowed,
            resource_refresh_fresh: true,
            remote_unknown_order_ids: vec![],
            observed_at,
            no_trading_side_effect: true,
        },
    )
    .await
    .expect("record reconcile backlog from order lifecycle");
    assert_eq!(receipt.evaluation.remote_unknown_orders, 1);
    assert!(receipt.evaluation.submit_blocked);
    assert!(!receipt.provider_tick.submit_allowed_by_runtime);

    let service = ExecutorService::with_runtime_provider(
        store.clone(),
        StoreBackedRuntimeStateProvider::new(store.clone()),
        "test-executor".into(),
        DEFAULT_CONTRACT_VERSION.into(),
    );
    let normalized = service.normalize(intent()).await.expect("normalize");
    let snapshot = service
        .capture_snapshot(normalized)
        .await
        .expect("snapshot");
    assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Degraded);
    assert!(
        snapshot
            .runtime_state
            .required_capabilities
            .contains(&"reconcile-backlog".to_string())
    );
}

#[tokio::test]
async fn service_records_websocket_liveness_worker_tick_for_decision_gate() {
    let store = InMemoryStore::default();
    store.set_runtime_state_for_test("acct-1", "cond-1", None, allow_runtime_state());
    for capability in ["heartbeat", "reconcile", "resource-refresh"] {
        store
            .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                worker_id: format!("worker-{capability}"),
                role: "service-test".into(),
                capability: capability.into(),
                status: "HEALTHY".into(),
                last_heartbeat_at: Utc::now(),
                last_error: None,
            })
            .await
            .expect("record worker heartbeat");
    }

    let observed_at = Utc::now();
    let receipt = record_websocket_liveness_worker_tick(
        &store,
        WebSocketLivenessWorkerTick {
            account_id: "acct-1".into(),
            provider_name: "websocket-liveness-worker-test".into(),
            instance_id: "worker-websocket-liveness".into(),
            lease_owner_id: "worker-websocket-liveness".into(),
            geoblock_status: GeoblockStatus::Allowed,
            resource_refresh_fresh: true,
            remote_unknown_orders: 0,
            observed_at,
            stale_after_seconds: 30,
            no_trading_side_effect: true,
            observations: vec![
                pmx_runtime::WebSocketLivenessObservation {
                    channel: pmx_runtime::WebSocketChannel::Market,
                    connected: true,
                    last_message_at: Some(observed_at - chrono::Duration::seconds(5)),
                    status: pmx_runtime::HealthLevel::Healthy,
                    last_error: None,
                },
                pmx_runtime::WebSocketLivenessObservation {
                    channel: pmx_runtime::WebSocketChannel::User,
                    connected: false,
                    last_message_at: None,
                    status: pmx_runtime::HealthLevel::Degraded,
                    last_error: Some("user websocket disconnected".into()),
                },
            ],
        },
    )
    .await
    .expect("record websocket liveness worker tick");
    assert!(receipt.evaluation.market_connected);
    assert!(!receipt.evaluation.user_connected);
    assert!(!receipt.provider_tick.submit_allowed_by_runtime);

    let service = ExecutorService::with_runtime_provider(
        store.clone(),
        StoreBackedRuntimeStateProvider::new(store.clone()),
        "test-executor".into(),
        DEFAULT_CONTRACT_VERSION.into(),
    );
    let normalized = service.normalize(intent()).await.expect("normalize");
    let snapshot = service
        .capture_snapshot(normalized.clone())
        .await
        .expect("snapshot");
    assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Degraded);
    assert!(
        snapshot
            .runtime_state
            .required_capabilities
            .contains(&"websocket:user".to_string())
    );
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
        })
        .await
        .expect("decision");
    assert_eq!(decision.status, DecisionStatus::Block);
    assert!(decision.reasons.contains(&BlockReason::WorkerDegraded));
}

#[tokio::test]
async fn service_records_geoblock_worker_tick_for_decision_gate() {
    let store = InMemoryStore::default();
    store.set_runtime_state_for_test("acct-1", "cond-1", None, allow_runtime_state());
    for capability in ["heartbeat", "reconcile", "resource-refresh"] {
        store
            .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                worker_id: format!("worker-{capability}"),
                role: "service-test".into(),
                capability: capability.into(),
                status: "HEALTHY".into(),
                last_heartbeat_at: Utc::now(),
                last_error: None,
            })
            .await
            .expect("record worker heartbeat");
    }

    let receipt = record_geoblock_worker_tick(
        &store,
        GeoblockWorkerTick {
            account_id: "acct-1".into(),
            provider_name: "geoblock-worker-test".into(),
            instance_id: "worker-geoblock".into(),
            lease_owner_id: "worker-geoblock".into(),
            market_websocket_connected: true,
            market_websocket_stale: false,
            user_websocket_connected: true,
            user_websocket_stale: false,
            status: GeoblockStatus::Unknown,
            resource_refresh_fresh: true,
            remote_unknown_orders: 0,
            observed_at: Utc::now(),
            last_error: Some("geoblock provider timeout".into()),
            no_trading_side_effect: true,
        },
    )
    .await
    .expect("record geoblock worker tick");
    assert!(!receipt.evaluation.submit_allowed);
    assert!(!receipt.provider_tick.submit_allowed_by_runtime);

    let service = ExecutorService::with_runtime_provider(
        store.clone(),
        StoreBackedRuntimeStateProvider::new(store.clone()),
        "test-executor".into(),
        DEFAULT_CONTRACT_VERSION.into(),
    );
    let normalized = service.normalize(intent()).await.expect("normalize");
    let snapshot = service
        .capture_snapshot(normalized.clone())
        .await
        .expect("snapshot");
    assert_eq!(
        snapshot.runtime_state.geoblock_status,
        GeoblockStatus::Allowed
    );
    assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Unknown);
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
        })
        .await
        .expect("decision");
    assert_eq!(decision.status, DecisionStatus::Block);
    assert!(decision.reasons.contains(&BlockReason::WorkerUnknown));
}

#[tokio::test]
async fn service_records_worker_crash_recovery_tick_for_decision_gate() {
    let store = InMemoryStore::default();
    store.set_runtime_state_for_test("acct-1", "cond-1", None, allow_runtime_state());
    for capability in ["heartbeat", "reconcile", "resource-refresh"] {
        store
            .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                worker_id: format!("worker-{capability}"),
                role: "service-test".into(),
                capability: capability.into(),
                status: "HEALTHY".into(),
                last_heartbeat_at: Utc::now(),
                last_error: None,
            })
            .await
            .expect("record worker heartbeat");
    }

    let observed_at = Utc::now();
    let receipt = record_worker_crash_recovery_tick(
        &store,
        WorkerCrashRecoveryTick {
            account_id: "acct-1".into(),
            worker_id: "worker-crash-recovery".into(),
            required_capabilities: vec![
                "heartbeat".into(),
                "reconcile".into(),
                "resource-refresh".into(),
            ],
            observed_at,
            stale_after_seconds: 30,
            no_trading_side_effect: true,
            observations: vec![
                pmx_runtime::WorkerCrashRecoveryObservation {
                    worker_id: "worker-heartbeat".into(),
                    capability: "heartbeat".into(),
                    status: pmx_runtime::HealthLevel::Healthy,
                    last_heartbeat_at: Some(observed_at - chrono::Duration::seconds(5)),
                    last_error: None,
                },
                pmx_runtime::WorkerCrashRecoveryObservation {
                    worker_id: "worker-reconcile".into(),
                    capability: "reconcile".into(),
                    status: pmx_runtime::HealthLevel::Healthy,
                    last_heartbeat_at: Some(observed_at - chrono::Duration::seconds(60)),
                    last_error: Some("stale after crash".into()),
                },
            ],
        },
    )
    .await
    .expect("record worker crash recovery tick");
    assert!(receipt.heartbeat_recorded);
    assert!(receipt.observation_recorded);
    assert!(!receipt.evaluation.recovered);
    assert_eq!(receipt.evaluation.stale_workers, vec!["worker-reconcile"]);
    assert_eq!(
        receipt.evaluation.missing_capabilities,
        vec!["resource-refresh"]
    );

    let service = ExecutorService::with_runtime_provider(
        store.clone(),
        StoreBackedRuntimeStateProvider::new(store.clone()),
        "test-executor".into(),
        DEFAULT_CONTRACT_VERSION.into(),
    );
    let normalized = service.normalize(intent()).await.expect("normalize");
    let snapshot = service
        .capture_snapshot(normalized.clone())
        .await
        .expect("snapshot");
    assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Stale);
    assert!(
        snapshot
            .runtime_state
            .required_capabilities
            .contains(&"worker-crash-recovery".to_string())
    );
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
        })
        .await
        .expect("decision");
    assert_eq!(decision.status, DecisionStatus::Block);
    assert!(decision.reasons.contains(&BlockReason::WorkerStale));
}
