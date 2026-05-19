use pmx_core::{AccountId, DecimalString, ExecutionId};

use crate::{
    InMemoryStore, RealFundsCanaryLifecycleState, RealFundsCanaryRunRecord,
    RealFundsCanaryRunStore, StoreError,
};

fn hex() -> String {
    "a".repeat(64)
}

fn record(run_id: &str) -> RealFundsCanaryRunRecord {
    RealFundsCanaryRunRecord {
        run_id: run_id.into(),
        execution_id: ExecutionId("exec-real-funds-canary".into()),
        account_id: AccountId("acct-real-funds-canary".into()),
        approval_hash: hex(),
        idempotency_key: "idem-real-funds-canary".into(),
        artifact_sha256: hex(),
        evidence_manifest_sha256: hex(),
        market_id: "market-real-funds-canary".into(),
        token_id_hash: hex(),
        max_order_notional_usd: DecimalString("1.00".into()),
        max_daily_notional_usd: DecimalString("2.00".into()),
        order_notional_usd: DecimalString("0.50".into()),
        execution_style: "FOK_LIMIT_FILL".into(),
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
async fn real_funds_canary_replays_same_idempotency() {
    let store = InMemoryStore::default();
    let first = store
        .record_real_funds_canary_run(&record("run-1"))
        .await
        .expect("record canary run");
    let replay = store
        .record_real_funds_canary_run(&record("run-2"))
        .await
        .expect("replay canary run");
    assert_eq!(first.run_id, replay.run_id);
}

#[tokio::test]
async fn real_funds_canary_rejects_idempotency_payload_mismatch() {
    let store = InMemoryStore::default();
    store
        .record_real_funds_canary_run(&record("run-1"))
        .await
        .expect("record canary run");
    let mut changed = record("run-2");
    changed.order_notional_usd = DecimalString("0.75".into());
    let err = store
        .record_real_funds_canary_run(&changed)
        .await
        .expect_err("idempotency conflict");
    assert!(matches!(err, StoreError::Conflict(_)));
}

#[tokio::test]
async fn real_funds_canary_freezes_remote_unknown_and_blocks_downgrade() {
    let store = InMemoryStore::default();
    store
        .record_real_funds_canary_run(&record("run-1"))
        .await
        .expect("record canary run");
    let frozen = store
        .update_real_funds_canary_state(
            "run-1",
            RealFundsCanaryLifecycleState::RemoteUnknownFreeze,
            Some("remote unknown in drill".into()),
        )
        .await
        .expect("freeze canary run");
    assert_eq!(
        frozen.lifecycle_state,
        RealFundsCanaryLifecycleState::RemoteUnknownFreeze
    );
    let err = store
        .update_real_funds_canary_state(
            "run-1",
            RealFundsCanaryLifecycleState::ReadyButLiveDisabled,
            None,
        )
        .await
        .expect_err("downgrade must fail");
    assert!(matches!(err, StoreError::Conflict(_)));
}

#[tokio::test]
async fn real_funds_canary_rejects_raw_signed_exposure() {
    let store = InMemoryStore::default();
    let mut exposed = record("run-1");
    exposed.raw_signed_order_exposed = true;
    let err = store
        .record_real_funds_canary_run(&exposed)
        .await
        .expect_err("raw signed exposure must fail");
    assert!(matches!(err, StoreError::Conflict(_)));
}
