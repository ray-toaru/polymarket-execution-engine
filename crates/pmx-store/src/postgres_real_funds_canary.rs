use async_trait::async_trait;
use pmx_core::{AccountId, DecimalString, ExecutionId};
use tokio_postgres::Row;

use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{
    RealFundsCanaryLifecycleState, RealFundsCanaryRunRecord, RealFundsCanaryRunStore, StoreError,
    real_funds_canary_state_from_str, real_funds_canary_state_to_str,
    validate_real_funds_canary_transition,
};

#[async_trait]
impl RealFundsCanaryRunStore for PostgresStore {
    async fn record_real_funds_canary_run(
        &self,
        record: &RealFundsCanaryRunRecord,
    ) -> Result<RealFundsCanaryRunRecord, StoreError> {
        validate_record(record)?;
        let client = self.client().await?;
        let lifecycle_state = real_funds_canary_state_to_str(&record.lifecycle_state);
        let result = client
            .execute(
                "INSERT INTO real_funds_canary_runs
                 (run_id, execution_id, account_id, approval_hash, idempotency_key,
                  artifact_sha256, evidence_manifest_sha256, market_id, token_id_hash,
                  max_order_notional_usd, max_daily_notional_usd, order_notional_usd,
                  execution_style, remote_order_id, remote_status, lifecycle_state,
                  remote_side_effects, raw_signed_order_exposed)
                 VALUES
                 ($1, $2, $3, $4, $5, $6, $7, $8, $9,
                  $10::text::numeric, $11::text::numeric, $12::text::numeric,
                  $13, $14, $15, $16, $17, $18)
                 ON CONFLICT (account_id, idempotency_key) DO NOTHING",
                &[
                    &record.run_id,
                    &record.execution_id.0,
                    &record.account_id.0,
                    &record.approval_hash,
                    &record.idempotency_key,
                    &record.artifact_sha256,
                    &record.evidence_manifest_sha256,
                    &record.market_id,
                    &record.token_id_hash,
                    &record.max_order_notional_usd.0,
                    &record.max_daily_notional_usd.0,
                    &record.order_notional_usd.0,
                    &record.execution_style,
                    &record.remote_order_id,
                    &record.remote_status,
                    &lifecycle_state,
                    &record.remote_side_effects,
                    &record.raw_signed_order_exposed,
                ],
            )
            .await
            .map_err(map_db_error)?;
        let stored = self
            .load_real_funds_canary_run_by_idempotency(&record.account_id, &record.idempotency_key)
            .await?
            .ok_or_else(|| {
                StoreError::InvalidData("missing inserted real-funds canary run".into())
            })?;
        if result == 0 && !stored.same_idempotent_request(record) {
            return Err(StoreError::Conflict(
                "real-funds canary idempotency key reused with different request".into(),
            ));
        }
        Ok(stored)
    }

    async fn load_real_funds_canary_run(
        &self,
        run_id: &str,
    ) -> Result<Option<RealFundsCanaryRunRecord>, StoreError> {
        let client = self.client().await?;
        let row = client
            .query_opt(&select_sql("run_id = $1"), &[&run_id])
            .await
            .map_err(map_db_error)?;
        row.map(record_from_row).transpose()
    }

    async fn load_real_funds_canary_run_by_idempotency(
        &self,
        account_id: &AccountId,
        idempotency_key: &str,
    ) -> Result<Option<RealFundsCanaryRunRecord>, StoreError> {
        let client = self.client().await?;
        let row = client
            .query_opt(
                &select_sql("account_id = $1 AND idempotency_key = $2"),
                &[&account_id.0, &idempotency_key],
            )
            .await
            .map_err(map_db_error)?;
        row.map(record_from_row).transpose()
    }

    async fn update_real_funds_canary_state(
        &self,
        run_id: &str,
        lifecycle_state: RealFundsCanaryLifecycleState,
        remote_status: Option<String>,
    ) -> Result<RealFundsCanaryRunRecord, StoreError> {
        let existing = self
            .load_real_funds_canary_run(run_id)
            .await?
            .ok_or_else(|| StoreError::NotFound(format!("real-funds canary run: {run_id}")))?;
        validate_real_funds_canary_transition(&existing.lifecycle_state, &lifecycle_state)?;
        let client = self.client().await?;
        let state = real_funds_canary_state_to_str(&lifecycle_state);
        let row = client
            .query_one(
                &format!(
                    "{}
                     UPDATE real_funds_canary_runs
                     SET lifecycle_state = $2, remote_status = $3, updated_at = now()
                     WHERE run_id = $1
                     RETURNING {}",
                    "",
                    select_columns()
                ),
                &[&run_id, &state, &remote_status],
            )
            .await
            .map_err(map_db_error)?;
        record_from_row(row)
    }
}

fn validate_record(record: &RealFundsCanaryRunRecord) -> Result<(), StoreError> {
    if record.raw_signed_order_exposed {
        return Err(StoreError::Conflict(
            "real-funds canary must not expose raw signed order material".into(),
        ));
    }
    if record.remote_side_effects {
        return Err(StoreError::Conflict(
            "local real-funds canary lifecycle records must not mark remote side effects".into(),
        ));
    }
    Ok(())
}

fn select_sql(predicate: &str) -> String {
    format!(
        "SELECT {} FROM real_funds_canary_runs WHERE {}",
        select_columns(),
        predicate
    )
}

fn select_columns() -> &'static str {
    "run_id, execution_id, account_id, approval_hash, idempotency_key,
     artifact_sha256, evidence_manifest_sha256, market_id, token_id_hash,
     max_order_notional_usd::text, max_daily_notional_usd::text, order_notional_usd::text,
     execution_style, remote_order_id, remote_status, lifecycle_state,
     remote_side_effects, raw_signed_order_exposed, created_at, updated_at"
}

fn record_from_row(row: Row) -> Result<RealFundsCanaryRunRecord, StoreError> {
    Ok(RealFundsCanaryRunRecord {
        run_id: row.get(0),
        execution_id: ExecutionId(row.get(1)),
        account_id: AccountId(row.get(2)),
        approval_hash: row.get(3),
        idempotency_key: row.get(4),
        artifact_sha256: row.get(5),
        evidence_manifest_sha256: row.get(6),
        market_id: row.get(7),
        token_id_hash: row.get(8),
        max_order_notional_usd: DecimalString(row.get(9)),
        max_daily_notional_usd: DecimalString(row.get(10)),
        order_notional_usd: DecimalString(row.get(11)),
        execution_style: row.get(12),
        remote_order_id: row.get(13),
        remote_status: row.get(14),
        lifecycle_state: real_funds_canary_state_from_str(row.get::<_, String>(15).as_str())?,
        remote_side_effects: row.get(16),
        raw_signed_order_exposed: row.get(17),
        created_at: row.get(18),
        updated_at: row.get(19),
    })
}
