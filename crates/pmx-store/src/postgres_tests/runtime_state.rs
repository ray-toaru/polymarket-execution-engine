use super::*;
use pmx_core::{CollateralProfileStatus, GeoblockStatus, WorkerStatus};

#[tokio::test]
async fn postgres_loads_runtime_state_from_runtime_tables() {
    let Some(store) = test_store().await else {
        return;
    };
    let account = unique("acct-runtime");
    let condition = unique("cond-runtime");
    let profile = unique("profile-runtime");
    let client = store.client().await.expect("test postgres client");
    client
        .execute(
            "INSERT INTO runtime_accounts (account_id, status, kill_switch_enabled) VALUES ($1, 'ACTIVE', false)",
            &[&account],
        )
        .await
        .expect("seed runtime account");
    client
        .execute(
            "INSERT INTO runtime_markets (condition_id, status, is_sports) VALUES ($1, 'ACTIVE', false)",
            &[&condition],
        )
        .await
        .expect("seed runtime market");
    client
        .execute(
            "INSERT INTO collateral_profiles (profile_id, status, quote_asset_symbol, quote_asset_address, allowance_target, decimals, profile_version) \
             VALUES ($1, 'RESOLVED', 'pUSD', '0x0000000000000000000000000000000000000001', '0x0000000000000000000000000000000000000002', 6, 'test')",
            &[&profile],
        )
        .await
        .expect("seed collateral profile");
    for capability in ["heartbeat", "reconcile", "resource-refresh"] {
        let worker_id = unique(&format!("worker-{capability}"));
        let capability_value = capability.to_string();
        client
            .execute(
                "INSERT INTO worker_health (worker_id, role, capability, status, last_heartbeat_at) \
                 VALUES ($1, 'test', $2, 'HEALTHY', now())",
                &[&worker_id, &capability_value],
            )
            .await
            .expect("seed worker health");
    }
    let state = store
        .load_runtime_state(&RuntimeStateQuery {
            account_id: account,
            condition_id: condition,
            collateral_profile_id: Some(profile),
            required_capabilities: vec![
                "heartbeat".into(),
                "reconcile".into(),
                "resource-refresh".into(),
            ],
        })
        .await
        .expect("runtime state");
    assert_eq!(state.geoblock_status, GeoblockStatus::Allowed);
    assert_eq!(state.worker_status, WorkerStatus::Healthy);
    assert_eq!(
        state.collateral_profile_status,
        CollateralProfileStatus::Resolved
    );
    assert!(!state.kill_switch_enabled);
}

#[tokio::test]
async fn postgres_runtime_worker_observations_degrade_runtime_state() {
    let Some(store) = test_store().await else {
        return;
    };
    let account = unique("acct-runtime-observed");
    let condition = unique("cond-runtime-observed");
    let profile = unique("profile-runtime-observed");
    let client = store.client().await.expect("test postgres client");
    client
        .execute(
            "INSERT INTO runtime_accounts (account_id, status, kill_switch_enabled) VALUES ($1, 'ACTIVE', false)",
            &[&account],
        )
        .await
        .expect("seed runtime account");
    client
        .execute(
            "INSERT INTO runtime_markets (condition_id, status, is_sports) VALUES ($1, 'ACTIVE', false)",
            &[&condition],
        )
        .await
        .expect("seed runtime market");
    client
        .execute(
            "INSERT INTO collateral_profiles (profile_id, status, quote_asset_symbol, quote_asset_address, allowance_target, decimals, profile_version) \
             VALUES ($1, 'RESOLVED', 'pUSD', '0x0000000000000000000000000000000000000001', '0x0000000000000000000000000000000000000002', 6, 'test')",
            &[&profile],
        )
        .await
        .expect("seed collateral profile");
    for capability in ["heartbeat", "reconcile", "resource-refresh"] {
        let worker_id = unique(&format!("worker-{capability}"));
        let capability_value = capability.to_string();
        client
            .execute(
                "INSERT INTO worker_health (worker_id, role, capability, status, last_heartbeat_at) \
                 VALUES ($1, 'test', $2, 'HEALTHY', now())",
                &[&worker_id, &capability_value],
            )
            .await
            .expect("seed worker health");
    }
    store
        .record_runtime_worker_observation(&RuntimeWorkerObservation {
            account_id: account.clone(),
            capability: "heartbeat-lease".into(),
            worker_kind: "HeartbeatLease".into(),
            status: "STALE".into(),
            should_fail_closed: true,
            reason: "lease expired".into(),
            observed_at: None,
        })
        .await
        .expect("record runtime worker observation");
    let state = store
        .load_runtime_state(&RuntimeStateQuery {
            account_id: account,
            condition_id: condition,
            collateral_profile_id: Some(profile),
            required_capabilities: vec![
                "heartbeat".into(),
                "reconcile".into(),
                "resource-refresh".into(),
            ],
        })
        .await
        .expect("runtime state");
    assert_eq!(state.worker_status, WorkerStatus::Stale);
    assert!(
        state
            .required_capabilities
            .contains(&"heartbeat-lease".into())
    );
}

#[tokio::test]
async fn postgres_records_runtime_worker_observation() {
    let Some(store) = test_store().await else {
        return;
    };
    let account = unique("acct-worker-observation");
    store
        .record_runtime_worker_observation(&RuntimeWorkerObservation {
            account_id: account.clone(),
            capability: "heartbeat-lease".into(),
            worker_kind: "HeartbeatLease".into(),
            status: "STALE".into(),
            should_fail_closed: true,
            reason: "lease expired".into(),
            observed_at: None,
        })
        .await
        .expect("record runtime worker observation");
    let client = store.client().await.expect("test postgres client");
    let count: i64 = client
        .query_one(
            "SELECT COUNT(*)::bigint FROM runtime_worker_observations WHERE account_id = $1",
            &[&account],
        )
        .await
        .expect("count runtime worker observations")
        .get(0);
    assert_eq!(count, 1);
}
