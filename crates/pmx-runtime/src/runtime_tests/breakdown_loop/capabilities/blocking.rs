use super::super::super::*;

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
