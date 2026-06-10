use super::super::*;
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
async fn postgres_global_kill_switch_overrides_account_runtime_state() {
    let Some(store) = test_store().await else {
        return;
    };
    let account = unique("acct-runtime-global-kill");
    let condition = unique("cond-runtime-global-kill");
    let profile = unique("profile-runtime-global-kill");
    let client = store.client().await.expect("test postgres client");
    store
        .set_global_kill_switch(false, "clear global test state")
        .await
        .expect("clear global kill switch");
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
    let query = RuntimeStateQuery {
        account_id: account,
        condition_id: condition,
        collateral_profile_id: Some(profile),
        required_capabilities: vec![],
    };
    let state = store
        .load_runtime_state(&query)
        .await
        .expect("runtime state before global kill");
    assert!(!state.kill_switch_enabled);

    store
        .set_global_kill_switch(true, "global test block")
        .await
        .expect("set global kill switch");
    let state = store
        .load_runtime_state(&query)
        .await
        .expect("runtime state after global kill");
    assert!(state.kill_switch_enabled);

    store
        .set_global_kill_switch(false, "clear global test block")
        .await
        .expect("clear global kill switch");
    let state = store
        .load_runtime_state(&query)
        .await
        .expect("runtime state after global clear");
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
async fn postgres_loads_canary_runtime_truth_from_runtime_rows() {
    let Some(store) = test_store().await else {
        return;
    };
    let account = unique("acct-canary-truth");
    let condition = unique("cond-canary-truth");
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
    for capability in [
        "heartbeat",
        "reconcile",
        "resource-refresh",
        "live-submit-gate",
        "idempotency-lease",
        "order-cancel-reconciliation",
        "repository-reservation",
        "reconcile-worker",
        "cancel-only-fallback",
        "balance-allowance-check",
    ] {
        client
            .execute(
                "INSERT INTO worker_health (worker_id, role, capability, status, last_heartbeat_at) \
                 VALUES ($1, 'CanaryRuntimeTruth', $2, 'HEALTHY', now())",
                &[&unique(&format!("worker-{capability}")), &capability],
            )
            .await
            .expect("seed canary runtime truth worker");
    }

    let truth = store
        .load_canary_runtime_truth(&CanaryRuntimeTruthQuery {
            account_id: account.clone(),
            condition_id: condition,
            collateral_profile_id: None,
        })
        .await
        .expect("load canary runtime truth");
    assert!(truth.all_ready());
    assert_eq!(truth.runtime_worker_healthy, Some(true));
    assert_eq!(truth.geoblock_allowed, Some(true));
    assert_eq!(truth.repository_reservation_exists, Some(true));
    assert_eq!(truth.idempotency_key_written, Some(true));
    assert_eq!(truth.reconcile_worker_healthy, Some(true));
    assert_eq!(truth.cancel_only_fallback_ready, Some(true));
    assert_eq!(truth.balance_allowance_checked, Some(true));

    store
        .record_runtime_worker_observation(&RuntimeWorkerObservation {
            account_id: account.clone(),
            capability: "order-cancel-reconciliation".into(),
            worker_kind: "CanaryRuntimeTruth".into(),
            status: "BLOCKED".into(),
            should_fail_closed: true,
            reason: "cancel reconciliation stale".into(),
            observed_at: None,
        })
        .await
        .expect("record blocking observation");
    let truth = store
        .load_canary_runtime_truth(&CanaryRuntimeTruthQuery {
            account_id: account,
            condition_id: unique("cond-canary-truth-recheck"),
            collateral_profile_id: None,
        })
        .await
        .expect("load blocked canary runtime truth");
    assert!(!truth.order_cancel_reconciliation_ready);
    assert_eq!(truth.runtime_worker_healthy, Some(true));
    assert_eq!(truth.geoblock_allowed, Some(true));
    assert!(!truth.all_ready());
}

#[tokio::test]
async fn postgres_runtime_state_prefers_scoped_collateral_and_worker_rows() {
    let Some(store) = test_store().await else {
        return;
    };
    let account = unique("acct-runtime-scoped");
    let other_account = unique("acct-runtime-scoped-other");
    let condition = unique("cond-runtime-scoped");
    let profile = unique("profile-runtime-scoped");
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
            "INSERT INTO collateral_profiles (profile_id, status, quote_asset_symbol, quote_asset_address, allowance_target, decimals, profile_version, account_id, condition_id) \
             VALUES ($1, 'RESOLVED', 'pUSD', '0x0000000000000000000000000000000000000003', '0x0000000000000000000000000000000000000004', 6, 'scoped', $2, $3)",
            &[&profile, &account, &condition],
        )
        .await
        .expect("seed scoped collateral profile");
    client
        .execute(
            "INSERT INTO worker_health (worker_id, role, capability, status, last_heartbeat_at) \
             VALUES ($1, 'test', 'reconcile-scoped', 'HEALTHY', now())",
            &[&unique("worker-global-reconcile-scoped")],
        )
        .await
        .expect("seed global worker health");
    client
        .execute(
            "INSERT INTO worker_health (worker_id, role, capability, status, last_heartbeat_at, account_id, condition_id) \
             VALUES ($1, 'test', 'reconcile-scoped', 'DEGRADED', now(), $2, $3)",
            &[&unique("worker-account-reconcile-scoped"), &account, &condition],
        )
        .await
        .expect("seed scoped worker health");

    let scoped_state = store
        .load_runtime_state(&RuntimeStateQuery {
            account_id: account,
            condition_id: condition.clone(),
            collateral_profile_id: Some(profile.clone()),
            required_capabilities: vec!["reconcile-scoped".into()],
        })
        .await
        .expect("scoped runtime state");
    assert_eq!(
        scoped_state.collateral_profile_status,
        CollateralProfileStatus::Resolved
    );
    assert_eq!(scoped_state.worker_status, WorkerStatus::Degraded);

    let fallback_state = store
        .load_runtime_state(&RuntimeStateQuery {
            account_id: other_account,
            condition_id: condition,
            collateral_profile_id: Some(profile),
            required_capabilities: vec!["reconcile-scoped".into()],
        })
        .await
        .expect("fallback runtime state");
    assert_eq!(
        fallback_state.collateral_profile_status,
        CollateralProfileStatus::ExplicitMissing
    );
    assert_eq!(fallback_state.worker_status, WorkerStatus::Healthy);
}
