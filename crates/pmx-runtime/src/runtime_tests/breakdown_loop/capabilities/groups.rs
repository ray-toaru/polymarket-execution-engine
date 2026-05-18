use super::super::super::*;

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
