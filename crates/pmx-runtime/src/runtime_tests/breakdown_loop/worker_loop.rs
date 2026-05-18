use super::super::*;

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
