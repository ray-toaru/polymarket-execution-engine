use super::*;

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SnapshotHashInput<'a> {
    pub snapshot_id: &'a str,
    pub normalized_intent_id: &'a str,
    pub runtime_state: &'a RuntimeStateSummary,
    pub captured_at: DateTime<Utc>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanHashInput<'a> {
    account_id: &'a AccountId,
    normalized_intent_id: &'a str,
    snapshot_id: &'a str,
    snapshot_hash: &'a HashValue,
    decision_id: &'a str,
    decision_hash: &'a HashValue,
    approval_id: &'a str,
    approval_hash: &'a HashValue,
    status: &'a PlanStatus,
    condition_id: &'a ConditionId,
    token_id: &'a TokenId,
    side: &'a Side,
    quantity_bound: &'a QuantityBound,
    limit_price: &'a DecimalString,
    time_in_force: &'a TimeInForce,
    collateral_profile_id: &'a Option<String>,
    max_exposure: &'a DecimalString,
    executor_version: &'a str,
    contract_version: &'a str,
}

impl<'a> From<&'a ExecutionPlanSummary> for PlanHashInput<'a> {
    fn from(plan: &'a ExecutionPlanSummary) -> Self {
        Self {
            account_id: &plan.account_id,
            normalized_intent_id: &plan.normalized_intent_id,
            snapshot_id: &plan.snapshot_id,
            snapshot_hash: &plan.snapshot_hash,
            decision_id: &plan.decision_id,
            decision_hash: &plan.decision_hash,
            approval_id: &plan.approval_id,
            approval_hash: &plan.approval_hash,
            status: &plan.status,
            condition_id: &plan.condition_id,
            token_id: &plan.token_id,
            side: &plan.side,
            quantity_bound: &plan.quantity_bound,
            limit_price: &plan.limit_price,
            time_in_force: &plan.time_in_force,
            collateral_profile_id: &plan.collateral_profile_id,
            max_exposure: &plan.max_exposure,
            executor_version: &plan.executor_version,
            contract_version: &plan.contract_version,
        }
    }
}
