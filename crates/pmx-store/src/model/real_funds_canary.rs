use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pmx_core::{AccountId, DecimalString, ExecutionId};
use serde::{Deserialize, Serialize};

use super::StoreError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum RealFundsCanaryLifecycleState {
    PreflightReady,
    BlockedPrecheckFailed,
    ReadyButLiveDisabled,
    RemoteUnknownFreeze,
    OperatorRequired,
    SimulatedReconciled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealFundsCanaryRunRecord {
    pub run_id: String,
    pub execution_id: ExecutionId,
    pub account_id: AccountId,
    pub approval_hash: String,
    pub idempotency_key: String,
    pub artifact_sha256: String,
    pub evidence_manifest_sha256: String,
    pub market_id: String,
    pub token_id_hash: String,
    pub max_order_notional_usd: DecimalString,
    pub max_daily_notional_usd: DecimalString,
    pub order_notional_usd: DecimalString,
    pub execution_style: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_order_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_status: Option<String>,
    pub lifecycle_state: RealFundsCanaryLifecycleState,
    pub remote_side_effects: bool,
    pub raw_signed_order_exposed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
}

impl RealFundsCanaryRunRecord {
    pub fn same_idempotent_request(&self, other: &Self) -> bool {
        self.account_id == other.account_id
            && self.idempotency_key == other.idempotency_key
            && self.execution_id == other.execution_id
            && self.approval_hash == other.approval_hash
            && self.artifact_sha256 == other.artifact_sha256
            && self.evidence_manifest_sha256 == other.evidence_manifest_sha256
            && self.market_id == other.market_id
            && self.token_id_hash == other.token_id_hash
            && self.max_order_notional_usd == other.max_order_notional_usd
            && self.max_daily_notional_usd == other.max_daily_notional_usd
            && self.order_notional_usd == other.order_notional_usd
            && self.execution_style == other.execution_style
            && self.remote_side_effects == other.remote_side_effects
            && self.raw_signed_order_exposed == other.raw_signed_order_exposed
    }
}

#[async_trait]
pub trait RealFundsCanaryRunStore: Send + Sync {
    async fn record_real_funds_canary_run(
        &self,
        record: &RealFundsCanaryRunRecord,
    ) -> Result<RealFundsCanaryRunRecord, StoreError>;

    async fn load_real_funds_canary_run(
        &self,
        run_id: &str,
    ) -> Result<Option<RealFundsCanaryRunRecord>, StoreError>;

    async fn load_real_funds_canary_run_by_idempotency(
        &self,
        account_id: &AccountId,
        idempotency_key: &str,
    ) -> Result<Option<RealFundsCanaryRunRecord>, StoreError>;

    async fn update_real_funds_canary_state(
        &self,
        run_id: &str,
        lifecycle_state: RealFundsCanaryLifecycleState,
        remote_status: Option<String>,
    ) -> Result<RealFundsCanaryRunRecord, StoreError>;
}
