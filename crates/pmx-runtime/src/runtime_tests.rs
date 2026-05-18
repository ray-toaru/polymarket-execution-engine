use super::*;

#[test]
fn blocking_capabilities_include_submit_required_stale_worker() {
    let breakdown = RuntimeHealthBreakdown {
        account_id: "acct-1".into(),
        account_capabilities: vec![],
        market_capabilities: vec![],
        asset_capabilities: vec![],
        worker_capabilities: vec![CapabilityHealth {
            capability: "heartbeat".into(),
            required_for_submit: true,
            level: HealthLevel::Stale,
            last_observed_at: None,
            last_error: Some("missed heartbeat".into()),
        }],
    };
    assert_eq!(breakdown.blocking_capabilities().len(), 1);
}

#[test]
fn runtime_signals_map_websocket_and_heartbeat_to_submit_blockers() {
    let breakdown = runtime_breakdown_from_signals(
        "acct-runtime-v20",
        &[
            RuntimeSignal::WebSocket {
                channel: WebSocketChannel::Market,
                connected: true,
                stale: true,
                last_observed_at: None,
                last_error: Some("market stream stale".into()),
            },
            RuntimeSignal::HeartbeatLease {
                active: false,
                last_observed_at: None,
                last_error: Some("lease expired".into()),
            },
        ],
    );
    let blockers = breakdown.blocking_capabilities();
    assert_eq!(blockers.len(), 2);
    assert!(blockers.iter().any(|h| h.capability == "websocket:market"));
    assert!(blockers.iter().any(|h| h.capability == "heartbeat-lease"));
}

#[test]
fn geoblock_unknown_and_reconcile_backlog_block_submit() {
    let breakdown = runtime_breakdown_from_signals(
        "acct-runtime-v20",
        &[
            RuntimeSignal::Geoblock {
                status: GeoblockStatus::Unknown,
                last_observed_at: None,
                last_error: Some("provider timeout".into()),
            },
            RuntimeSignal::ReconcileBacklog {
                remote_unknown_orders: 1,
                last_observed_at: None,
                last_error: None,
            },
            RuntimeSignal::ResourceRefresh {
                fresh: false,
                last_observed_at: None,
                last_error: Some("resource refresh stale".into()),
            },
        ],
    );
    let names: Vec<_> = breakdown
        .blocking_capabilities()
        .into_iter()
        .map(|h| h.capability.as_str())
        .collect();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"geoblock"));
    assert!(names.contains(&"resource-refresh"));
    assert!(names.contains(&"reconcile-backlog"));
}

#[test]
fn worker_actions_mark_stale_runtime_inputs_as_fail_closed_updates() {
    let actions = worker_actions_from_runtime_signals(&[
        RuntimeSignal::HeartbeatLease {
            active: false,
            last_observed_at: None,
            last_error: Some("lease expired".into()),
        },
        RuntimeSignal::WebSocket {
            channel: WebSocketChannel::User,
            connected: false,
            stale: true,
            last_observed_at: None,
            last_error: Some("user stream disconnected".into()),
        },
    ]);
    assert_eq!(actions.len(), 2);
    assert!(actions.iter().all(|action| action.should_fail_closed));
    assert!(
        actions
            .iter()
            .all(|action| action.should_update_runtime_store)
    );
    assert!(
        actions
            .iter()
            .any(|action| action.capability == "heartbeat-lease")
    );
    assert!(
        actions
            .iter()
            .any(|action| action.capability == "websocket:user")
    );
}

#[test]
fn degraded_non_required_capability_does_not_block_submit() {
    let health = CapabilityHealth {
        capability: "reservation-sweeper".into(),
        required_for_submit: false,
        level: HealthLevel::Degraded,
        last_observed_at: None,
        last_error: Some("behind schedule".into()),
    };
    assert!(!health.blocks_submit());
}

#[test]
fn all_capabilities_preserves_account_market_asset_worker_groups() {
    let mk = |capability: &str| CapabilityHealth {
        capability: capability.into(),
        required_for_submit: true,
        level: HealthLevel::Healthy,
        last_observed_at: None,
        last_error: None,
    };
    let breakdown = RuntimeHealthBreakdown {
        account_id: "acct-runtime-v07".into(),
        account_capabilities: vec![mk("account-runtime")],
        market_capabilities: vec![mk("market-ws")],
        asset_capabilities: vec![mk("collateral-profile")],
        worker_capabilities: vec![mk("heartbeat-worker")],
    };
    let names: Vec<_> = breakdown
        .all_capabilities()
        .into_iter()
        .map(|health| health.capability.as_str())
        .collect();
    assert_eq!(
        names,
        vec![
            "account-runtime",
            "market-ws",
            "collateral-profile",
            "heartbeat-worker"
        ]
    );
}

#[test]
fn runtime_worker_store_writes_are_fail_closed_for_bad_signals() {
    let writes = runtime_worker_store_writes(
        "acct-worker-write",
        &[
            RuntimeSignal::Geoblock {
                status: GeoblockStatus::Unknown,
                last_observed_at: None,
                last_error: Some("geoblock unavailable".into()),
            },
            RuntimeSignal::HeartbeatLease {
                active: false,
                last_observed_at: None,
                last_error: Some("heartbeat lease expired".into()),
            },
            RuntimeSignal::ResourceRefresh {
                fresh: false,
                last_observed_at: None,
                last_error: Some("resource refresh stale".into()),
            },
        ],
    );
    assert_eq!(writes.len(), 3);
    assert!(writes.iter().all(|write| write.should_fail_closed));
    assert!(writes.iter().any(|write| write.capability == "geoblock"));
    assert!(
        writes
            .iter()
            .any(|write| write.capability == "heartbeat-lease")
    );
    assert!(
        writes
            .iter()
            .any(|write| write.capability == "resource-refresh")
    );
}

#[test]
fn runtime_worker_loop_tick_blocks_stale_down_and_geoblocked_submit() {
    let tick = runtime_worker_loop_tick(RuntimeWorkerLoopInput {
        account_id: "acct-runtime-loop".into(),
        lease_owner_id: "worker-a".into(),
        instance_id: "worker-b".into(),
        market_websocket_connected: false,
        market_websocket_stale: true,
        user_websocket_connected: true,
        user_websocket_stale: true,
        geoblock_status: GeoblockStatus::Blocked,
        resource_refresh_fresh: false,
        remote_unknown_orders: 2,
        observed_at: Utc::now(),
    });
    assert!(!tick.lease_owner_active);
    assert!(!tick.submit_allowed_by_runtime);
    assert_eq!(tick.signals.len(), 6);
    assert!(
        tick.actions
            .iter()
            .any(|action| action.capability == "heartbeat-lease" && action.should_fail_closed)
    );
    assert!(
        tick.actions
            .iter()
            .any(|action| action.capability == "websocket:market" && action.should_fail_closed)
    );
    assert!(
        tick.actions
            .iter()
            .any(|action| action.capability == "geoblock" && action.should_fail_closed)
    );
    assert!(
        tick.actions
            .iter()
            .any(|action| action.capability == "resource-refresh" && action.should_fail_closed)
    );
    assert!(
        tick.actions
            .iter()
            .any(|action| action.capability == "reconcile-backlog" && action.should_fail_closed)
    );
}

#[test]
fn runtime_worker_loop_tick_recovers_only_after_all_required_inputs_are_healthy() {
    let tick = runtime_worker_loop_tick(RuntimeWorkerLoopInput {
        account_id: "acct-runtime-loop".into(),
        lease_owner_id: "worker-a".into(),
        instance_id: "worker-a".into(),
        market_websocket_connected: true,
        market_websocket_stale: false,
        user_websocket_connected: true,
        user_websocket_stale: false,
        geoblock_status: GeoblockStatus::Allowed,
        resource_refresh_fresh: true,
        remote_unknown_orders: 0,
        observed_at: Utc::now(),
    });
    assert!(tick.lease_owner_active);
    assert!(tick.submit_allowed_by_runtime);
    assert!(tick.actions.iter().all(|action| !action.should_fail_closed));
}

struct FakeRuntimeWorkerProvider(RuntimeWorkerProviderSnapshot);

impl RuntimeWorkerProvider for FakeRuntimeWorkerProvider {
    fn snapshot(&self) -> RuntimeWorkerProviderSnapshot {
        self.0.clone()
    }
}

#[test]
fn runtime_worker_provider_snapshot_feeds_loop_without_trading_side_effects() {
    let provider = FakeRuntimeWorkerProvider(RuntimeWorkerProviderSnapshot {
        account_id: "acct-provider".into(),
        lease_owner_id: "worker-1".into(),
        instance_id: "worker-1".into(),
        market_websocket_connected: true,
        market_websocket_stale: false,
        user_websocket_connected: false,
        user_websocket_stale: true,
        geoblock_status: GeoblockStatus::Allowed,
        resource_refresh_fresh: true,
        remote_unknown_orders: 0,
        observed_at: Utc::now(),
        provider_name: "fake-provider".into(),
        no_trading_side_effect: true,
    });
    let tick = runtime_worker_loop_tick_from_provider(&provider);
    assert!(!tick.submit_allowed_by_runtime);
    assert!(
        tick.actions
            .iter()
            .any(|action| action.capability == "websocket:user" && action.should_fail_closed)
    );
}

#[test]
fn heartbeat_lease_election_selects_fresh_owner_and_fails_closed_for_non_owner() {
    let observed_at = Utc::now();
    let election = elect_heartbeat_lease_owner(HeartbeatLeaseElectionInput {
        instance_id: "worker-b".into(),
        observed_at,
        stale_after_seconds: 30,
        candidates: vec![
            HeartbeatLeaseCandidate {
                worker_id: "worker-a".into(),
                status: HealthLevel::Healthy,
                last_heartbeat_at: observed_at - chrono::Duration::seconds(5),
                last_error: None,
            },
            HeartbeatLeaseCandidate {
                worker_id: "worker-b".into(),
                status: HealthLevel::Healthy,
                last_heartbeat_at: observed_at - chrono::Duration::seconds(10),
                last_error: None,
            },
        ],
    });
    assert_eq!(election.lease_owner_id, "worker-a");
    assert!(!election.lease_owner_active);
    assert!(election.fail_closed);
}

#[test]
fn heartbeat_lease_election_has_no_owner_when_all_candidates_are_stale() {
    let observed_at = Utc::now();
    let election = elect_heartbeat_lease_owner(HeartbeatLeaseElectionInput {
        instance_id: "worker-a".into(),
        observed_at,
        stale_after_seconds: 30,
        candidates: vec![HeartbeatLeaseCandidate {
            worker_id: "worker-a".into(),
            status: HealthLevel::Healthy,
            last_heartbeat_at: observed_at - chrono::Duration::seconds(60),
            last_error: Some("missed heartbeat".into()),
        }],
    });
    assert!(election.lease_owner_id.is_empty());
    assert!(election.fail_closed);
    assert_eq!(election.healthy_candidate_count, 0);
}

#[test]
fn resource_refresh_evaluation_accepts_fresh_healthy_observations() {
    let observed_at = Utc::now();
    let evaluation = evaluate_resource_refresh_freshness(ResourceRefreshEvaluationInput {
        observed_at,
        stale_after_seconds: 30,
        observations: vec![
            ResourceRefreshObservation {
                component: ResourceRefreshComponent::Account,
                resource_id: "acct-1".into(),
                refreshed_at: observed_at - chrono::Duration::seconds(5),
                status: HealthLevel::Healthy,
                last_error: None,
            },
            ResourceRefreshObservation {
                component: ResourceRefreshComponent::Market,
                resource_id: "cond-1".into(),
                refreshed_at: observed_at - chrono::Duration::seconds(10),
                status: HealthLevel::Healthy,
                last_error: None,
            },
            ResourceRefreshObservation {
                component: ResourceRefreshComponent::Collateral,
                resource_id: "collateral-1".into(),
                refreshed_at: observed_at - chrono::Duration::seconds(15),
                status: HealthLevel::Healthy,
                last_error: None,
            },
        ],
    });
    assert!(evaluation.fresh);
    assert!(evaluation.stale_components.is_empty());
    assert!(evaluation.failed_components.is_empty());
    assert!(evaluation.missing_components.is_empty());
}

#[test]
fn resource_refresh_evaluation_fails_closed_for_stale_failed_or_missing_inputs() {
    let observed_at = Utc::now();
    let missing = evaluate_resource_refresh_freshness(ResourceRefreshEvaluationInput {
        observed_at,
        stale_after_seconds: 30,
        observations: vec![],
    });
    assert!(!missing.fresh);
    assert_eq!(missing.reason, "no resource refresh observations");
    assert_eq!(
        missing.missing_components,
        vec!["account", "market", "collateral"]
    );

    let evaluation = evaluate_resource_refresh_freshness(ResourceRefreshEvaluationInput {
        observed_at,
        stale_after_seconds: 30,
        observations: vec![
            ResourceRefreshObservation {
                component: ResourceRefreshComponent::Account,
                resource_id: "acct-1".into(),
                refreshed_at: observed_at - chrono::Duration::seconds(31),
                status: HealthLevel::Healthy,
                last_error: None,
            },
            ResourceRefreshObservation {
                component: ResourceRefreshComponent::Collateral,
                resource_id: "collateral-1".into(),
                refreshed_at: observed_at - chrono::Duration::seconds(1),
                status: HealthLevel::Degraded,
                last_error: Some("balance refresh failed".into()),
            },
        ],
    });
    assert!(!evaluation.fresh);
    assert_eq!(evaluation.stale_components, vec!["account:acct-1"]);
    assert_eq!(
        evaluation.failed_components,
        vec!["collateral:collateral-1"]
    );
    assert_eq!(evaluation.missing_components, vec!["market"]);
}

#[test]
fn reconcile_backlog_evaluation_blocks_submit_for_remote_unknown_orders() {
    let evaluation = evaluate_reconcile_backlog(ReconcileBacklogEvaluationInput {
        remote_unknown_order_ids: vec!["order-1".into(), "order-2".into()],
        observed_at: Utc::now(),
    });
    assert_eq!(evaluation.remote_unknown_orders, 2);
    assert!(evaluation.submit_blocked);
    assert_eq!(evaluation.reason, "remote_unknown_orders=2");
}

#[test]
fn reconcile_backlog_evaluation_allows_submit_when_backlog_empty() {
    let evaluation = evaluate_reconcile_backlog(ReconcileBacklogEvaluationInput {
        remote_unknown_order_ids: vec![],
        observed_at: Utc::now(),
    });
    assert_eq!(evaluation.remote_unknown_orders, 0);
    assert!(!evaluation.submit_blocked);
    assert_eq!(evaluation.reason, "no remote unknown reconcile backlog");
}

#[test]
fn websocket_liveness_evaluation_accepts_fresh_market_and_user_channels() {
    let observed_at = Utc::now();
    let evaluation = evaluate_websocket_liveness(WebSocketLivenessEvaluationInput {
        observed_at,
        stale_after_seconds: 30,
        observations: vec![
            WebSocketLivenessObservation {
                channel: WebSocketChannel::Market,
                connected: true,
                last_message_at: Some(observed_at - chrono::Duration::seconds(5)),
                status: HealthLevel::Healthy,
                last_error: None,
            },
            WebSocketLivenessObservation {
                channel: WebSocketChannel::User,
                connected: true,
                last_message_at: Some(observed_at - chrono::Duration::seconds(10)),
                status: HealthLevel::Healthy,
                last_error: None,
            },
        ],
    });
    assert!(evaluation.market_connected);
    assert!(!evaluation.market_stale);
    assert!(evaluation.user_connected);
    assert!(!evaluation.user_stale);
    assert!(evaluation.missing_channels.is_empty());
}

#[test]
fn websocket_liveness_evaluation_fails_closed_for_disconnected_stale_or_missing_channels() {
    let observed_at = Utc::now();
    let evaluation = evaluate_websocket_liveness(WebSocketLivenessEvaluationInput {
        observed_at,
        stale_after_seconds: 30,
        observations: vec![WebSocketLivenessObservation {
            channel: WebSocketChannel::Market,
            connected: true,
            last_message_at: Some(observed_at - chrono::Duration::seconds(31)),
            status: HealthLevel::Healthy,
            last_error: None,
        }],
    });
    assert!(evaluation.market_connected);
    assert!(evaluation.market_stale);
    assert!(!evaluation.user_connected);
    assert!(evaluation.user_stale);
    assert_eq!(evaluation.missing_channels, vec!["user"]);
}

#[test]
fn geoblock_evaluation_only_allows_explicit_allowed_status() {
    let allowed = evaluate_geoblock_status(GeoblockEvaluationInput {
        status: GeoblockStatus::Allowed,
        observed_at: Utc::now(),
        last_error: None,
    });
    assert!(allowed.submit_allowed);

    let blocked = evaluate_geoblock_status(GeoblockEvaluationInput {
        status: GeoblockStatus::Unknown,
        observed_at: Utc::now(),
        last_error: Some("provider timeout".into()),
    });
    assert!(!blocked.submit_allowed);
    assert_eq!(blocked.reason, "provider timeout");
}

#[test]
fn worker_crash_recovery_evaluation_requires_fresh_healthy_required_workers() {
    let observed_at = Utc::now();
    let evaluation = evaluate_worker_crash_recovery(WorkerCrashRecoveryEvaluationInput {
        observed_at,
        stale_after_seconds: 30,
        required_capabilities: vec![
            "heartbeat".into(),
            "reconcile".into(),
            "resource-refresh".into(),
        ],
        observations: vec![
            WorkerCrashRecoveryObservation {
                worker_id: "worker-heartbeat".into(),
                capability: "heartbeat".into(),
                status: HealthLevel::Healthy,
                last_heartbeat_at: Some(observed_at - chrono::Duration::seconds(5)),
                last_error: None,
            },
            WorkerCrashRecoveryObservation {
                worker_id: "worker-reconcile".into(),
                capability: "reconcile".into(),
                status: HealthLevel::Stale,
                last_heartbeat_at: Some(observed_at - chrono::Duration::seconds(5)),
                last_error: Some("restart loop".into()),
            },
        ],
    });
    assert!(!evaluation.recovered);
    assert_eq!(evaluation.failed_workers, vec!["worker-reconcile"]);
    assert_eq!(evaluation.missing_capabilities, vec!["resource-refresh"]);
}

#[test]
fn worker_crash_recovery_evaluation_recovers_after_all_required_workers_are_fresh() {
    let observed_at = Utc::now();
    let evaluation = evaluate_worker_crash_recovery(WorkerCrashRecoveryEvaluationInput {
        observed_at,
        stale_after_seconds: 30,
        required_capabilities: vec!["heartbeat".into()],
        observations: vec![WorkerCrashRecoveryObservation {
            worker_id: "worker-heartbeat".into(),
            capability: "heartbeat".into(),
            status: HealthLevel::Healthy,
            last_heartbeat_at: Some(observed_at - chrono::Duration::seconds(1)),
            last_error: None,
        }],
    });
    assert!(evaluation.recovered);
    assert!(evaluation.missing_capabilities.is_empty());
    assert!(evaluation.stale_workers.is_empty());
    assert!(evaluation.failed_workers.is_empty());
}
