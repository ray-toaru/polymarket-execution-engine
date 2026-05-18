use super::super::*;

#[tokio::test]
async fn service_records_continuous_runtime_worker_ticks_fail_closed_on_any_bad_snapshot() {
    let store = InMemoryStore::default();
    let observed_at = Utc::now();
    let healthy_snapshot = pmx_runtime::RuntimeWorkerProviderSnapshot {
        account_id: "acct-1".into(),
        lease_owner_id: "worker-runtime-1".into(),
        instance_id: "worker-runtime-1".into(),
        market_websocket_connected: true,
        market_websocket_stale: false,
        user_websocket_connected: true,
        user_websocket_stale: false,
        geoblock_status: GeoblockStatus::Allowed,
        resource_refresh_fresh: true,
        remote_unknown_orders: 0,
        observed_at,
        provider_name: "runtime-provider-test".into(),
        no_trading_side_effect: true,
    };
    let stale_snapshot = pmx_runtime::RuntimeWorkerProviderSnapshot {
        instance_id: "worker-runtime-2".into(),
        market_websocket_stale: true,
        observed_at: observed_at + chrono::Duration::seconds(1),
        ..healthy_snapshot.clone()
    };

    let receipt = record_runtime_worker_continuous_tick(
        &store,
        RuntimeWorkerContinuousTick {
            snapshots: vec![healthy_snapshot, stale_snapshot],
            no_trading_side_effect: true,
        },
    )
    .await
    .expect("record continuous runtime ticks");

    assert_eq!(receipt.ticks_recorded.len(), 2);
    assert!(receipt.ticks_recorded[0].submit_allowed_by_runtime);
    assert!(!receipt.ticks_recorded[1].submit_allowed_by_runtime);
    assert!(!receipt.all_submit_allowed_by_runtime);
    assert!(receipt.no_trading_side_effect);

    let report = store
        .list_runtime_worker_status(&RuntimeWorkerStatusQuery {
            account_id: "acct-1".into(),
            limit: 100,
            before_observed_at: None,
        })
        .await
        .expect("list runtime worker status");
    assert_eq!(report.heartbeats.len(), 2);
    assert!(
        report
            .observations
            .iter()
            .any(|observation| observation.capability == "websocket:market"
                && observation.should_fail_closed)
    );
}
