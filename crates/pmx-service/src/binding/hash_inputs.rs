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
    execution_id: &'a str,
    account_id: &'a AccountId,
    normalized_intent_id: &'a str,
    snapshot_id: &'a str,
    decision_id: &'a str,
    status: &'a PlanStatus,
    max_exposure: &'a DecimalString,
}

impl<'a> From<&'a ExecutionPlanSummary> for PlanHashInput<'a> {
    fn from(plan: &'a ExecutionPlanSummary) -> Self {
        Self {
            execution_id: &plan.execution_id,
            account_id: &plan.account_id,
            normalized_intent_id: &plan.normalized_intent_id,
            snapshot_id: &plan.snapshot_id,
            decision_id: &plan.decision_id,
            status: &plan.status,
            max_exposure: &plan.max_exposure,
        }
    }
}
