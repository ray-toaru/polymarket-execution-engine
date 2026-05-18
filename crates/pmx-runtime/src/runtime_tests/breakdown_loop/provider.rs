use super::super::*;
use super::FakeRuntimeWorkerProvider;

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
