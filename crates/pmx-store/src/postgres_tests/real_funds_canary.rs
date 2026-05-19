use pmx_core::{AccountId, DecimalString, ExecutionId};

use crate::{
    RealFundsCanaryLifecycleState, RealFundsCanaryRunRecord, RealFundsCanaryRunStore, StoreError,
};

fn hex() -> String {
    "b".repeat(64)
}

fn record(run_id: &str, suffix: &str) -> RealFundsCanaryRunRecord {
    RealFundsCanaryRunRecord {
        run_id: run_id.into(),
        execution_id: ExecutionId(format!("exec-real-funds-canary-{suffix}")),
        account_id: AccountId(format!("acct-real-funds-canary-{suffix}")),
        approval_hash: hex(),
        idempotency_key: format!("idem-real-funds-canary-{suffix}"),
        artifact_sha256: hex(),
        evidence_manifest_sha256: hex(),
        market_id: format!("market-real-funds-canary-{suffix}"),
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
async fn postgres_real_funds_canary_persists_and_replays_idempotency() {
    let Some(store) = super::test_store().await else {
        return;
    };
    let suffix = super::unique("real-funds-canary");
    let first = record(&format!("run-{suffix}-1"), &suffix);
    let replay = record(&format!("run-{suffix}-2"), &suffix);
    let stored = store
        .record_real_funds_canary_run(&first)
        .await
        .expect("record canary run");
    let replayed = store
        .record_real_funds_canary_run(&replay)
        .await
        .expect("replay canary run");
    assert_eq!(stored.run_id, replayed.run_id);
    assert!(replayed.created_at.is_some());
}

#[tokio::test]
async fn postgres_real_funds_canary_rejects_idempotency_payload_mismatch() {
    let Some(store) = super::test_store().await else {
        return;
    };
    let suffix = super::unique("real-funds-canary-conflict");
    let first = record(&format!("run-{suffix}-1"), &suffix);
    store
        .record_real_funds_canary_run(&first)
        .await
        .expect("record canary run");
    let mut changed = record(&format!("run-{suffix}-2"), &suffix);
    changed.order_notional_usd = DecimalString("0.75".into());
    let err = store
        .record_real_funds_canary_run(&changed)
        .await
        .expect_err("idempotency conflict");
    assert!(matches!(err, StoreError::Conflict(_)));
}

#[tokio::test]
async fn postgres_real_funds_canary_freeze_blocks_downgrade() {
    let Some(store) = super::test_store().await else {
        return;
    };
    let suffix = super::unique("real-funds-canary-freeze");
    let run = record(&format!("run-{suffix}"), &suffix);
    store
        .record_real_funds_canary_run(&run)
        .await
        .expect("record canary run");
    let frozen = store
        .update_real_funds_canary_state(
            &run.run_id,
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
            &run.run_id,
            RealFundsCanaryLifecycleState::ReadyButLiveDisabled,
            None,
        )
        .await
        .expect_err("downgrade must fail");
    assert!(matches!(err, StoreError::Conflict(_)));
}
