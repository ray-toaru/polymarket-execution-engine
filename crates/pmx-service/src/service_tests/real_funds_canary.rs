use pmx_core::{AccountId, DecimalString, ExecutionId};
use pmx_store::{
    InMemoryStore, RealFundsCanaryLifecycleState, RealFundsCanaryRunRecord,
    RealFundsCanaryRunStore, StoreError,
};

use crate::{
    freeze_real_funds_canary_remote_unknown, mark_real_funds_canary_live_disabled,
    record_real_funds_canary_preflight, record_real_funds_canary_simulated_reconcile,
};

fn hex() -> String {
    "c".repeat(64)
}

fn record(run_id: &str) -> RealFundsCanaryRunRecord {
    RealFundsCanaryRunRecord {
        run_id: run_id.into(),
        execution_id: ExecutionId("exec-real-funds-canary-service".into()),
        account_id: AccountId("acct-real-funds-canary-service".into()),
        approval_hash: hex(),
        idempotency_key: "idem-real-funds-canary-service".into(),
        artifact_sha256: hex(),
        evidence_manifest_sha256: hex(),
        market_id: "market-real-funds-canary-service".into(),
        token_id_hash: hex(),
        max_order_notional_usd: DecimalString("1.00".into()),
        max_daily_notional_usd: DecimalString("2.00".into()),
        order_notional_usd: DecimalString("0.50".into()),
        execution_style: "GTC_LIMIT_POST_ONLY_CANCEL".into(),
        remote_order_id: None,
        remote_status: None,
        lifecycle_state: RealFundsCanaryLifecycleState::PreflightReady,
        remote_side_effects: false,
        raw_signed_order_exposed: false,
        created_at: None,
        updated_at: None,
    }
}

#[tokio::test]
async fn service_records_real_funds_canary_preflight_without_remote_side_effects() {
    let store = InMemoryStore::default();
    let stored = record_real_funds_canary_preflight(&store, &record("run-service-1"))
        .await
        .expect("record canary preflight");
    assert!(!stored.remote_side_effects);
    assert!(!stored.raw_signed_order_exposed);
    assert_eq!(
        stored.lifecycle_state,
        RealFundsCanaryLifecycleState::PreflightReady
    );
}

#[tokio::test]
async fn service_real_funds_canary_replay_returns_existing_run() {
    let store = InMemoryStore::default();
    let first = record_real_funds_canary_preflight(&store, &record("run-service-1"))
        .await
        .expect("record canary preflight");
    let replay = record_real_funds_canary_preflight(&store, &record("run-service-2"))
        .await
        .expect("replay canary preflight");
    assert_eq!(first.run_id, replay.run_id);
}

#[tokio::test]
async fn service_freeze_blocks_live_disabled_downgrade() {
    let store = InMemoryStore::default();
    record_real_funds_canary_preflight(&store, &record("run-service-1"))
        .await
        .expect("record canary preflight");
    freeze_real_funds_canary_remote_unknown(
        &store,
        "run-service-1",
        Some("remote unknown in drill".into()),
    )
    .await
    .expect("freeze canary run");
    let err = mark_real_funds_canary_live_disabled(&store, "run-service-1")
        .await
        .expect_err("downgrade must fail");
    assert!(matches!(err, StoreError::Conflict(_)));
}

#[tokio::test]
async fn service_simulated_reconcile_remains_no_remote_side_effect() {
    let store = InMemoryStore::default();
    record_real_funds_canary_preflight(&store, &record("run-service-1"))
        .await
        .expect("record canary preflight");
    let reconciled = record_real_funds_canary_simulated_reconcile(&store, "run-service-1")
        .await
        .expect("simulated reconcile");
    assert_eq!(
        reconciled.lifecycle_state,
        RealFundsCanaryLifecycleState::SimulatedReconciled
    );
    let loaded = store
        .load_real_funds_canary_run("run-service-1")
        .await
        .expect("load canary run")
        .expect("canary run exists");
    assert!(!loaded.remote_side_effects);
}
