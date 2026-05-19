use pmx_store::{
    RealFundsCanaryLifecycleState, RealFundsCanaryRunRecord, RealFundsCanaryRunStore, StoreError,
};

pub async fn record_real_funds_canary_preflight<S>(
    store: &S,
    record: &RealFundsCanaryRunRecord,
) -> Result<RealFundsCanaryRunRecord, StoreError>
where
    S: RealFundsCanaryRunStore,
{
    if record.remote_side_effects {
        return Err(StoreError::Conflict(
            "real-funds canary preflight must not record remote side effects".into(),
        ));
    }
    if record.raw_signed_order_exposed {
        return Err(StoreError::Conflict(
            "real-funds canary preflight must not expose raw signed order material".into(),
        ));
    }
    store.record_real_funds_canary_run(record).await
}

pub async fn freeze_real_funds_canary_remote_unknown<S>(
    store: &S,
    run_id: &str,
    remote_status: Option<String>,
) -> Result<RealFundsCanaryRunRecord, StoreError>
where
    S: RealFundsCanaryRunStore,
{
    store
        .update_real_funds_canary_state(
            run_id,
            RealFundsCanaryLifecycleState::RemoteUnknownFreeze,
            remote_status,
        )
        .await
}

pub async fn mark_real_funds_canary_live_disabled<S>(
    store: &S,
    run_id: &str,
) -> Result<RealFundsCanaryRunRecord, StoreError>
where
    S: RealFundsCanaryRunStore,
{
    store
        .update_real_funds_canary_state(
            run_id,
            RealFundsCanaryLifecycleState::ReadyButLiveDisabled,
            Some("live submit disabled by release policy".into()),
        )
        .await
}

pub async fn record_real_funds_canary_simulated_reconcile<S>(
    store: &S,
    run_id: &str,
) -> Result<RealFundsCanaryRunRecord, StoreError>
where
    S: RealFundsCanaryRunStore,
{
    store
        .update_real_funds_canary_state(
            run_id,
            RealFundsCanaryLifecycleState::SimulatedReconciled,
            Some("simulated reconcile complete without remote side effects".into()),
        )
        .await
}
