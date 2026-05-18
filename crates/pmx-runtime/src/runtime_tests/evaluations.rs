use super::*;

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
