use super::super::*;

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
