use chrono::{DateTime, Utc};
use pmx_core::*;
use pmx_policy::evaluate_constraints;
use serde::Serialize;

use crate::model::ServiceError;

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

pub(crate) fn validate_sign_only_lifecycle_append(
    existing: &[SignOnlyLifecycleRecord],
    record: &SignOnlyLifecycleRecord,
) -> Result<(), ServiceError> {
    if !record.no_remote_side_effect {
        return Err(ServiceError::BadRequest(
            "sign-only lifecycle record must not contain remote side effects".into(),
        ));
    }
    if existing
        .last()
        .map(|last| sign_only_lifecycle_records_equivalent(last, record))
        .unwrap_or(false)
    {
        return Ok(());
    }
    if let Some(first) = existing.first()
        && first.account_id != record.account_id
    {
        return Err(ServiceError::Conflict(
            "sign-only lifecycle account_id does not match existing execution history".into(),
        ));
    }
    let from = existing
        .last()
        .map(|event| event.state.clone())
        .unwrap_or(SignOnlyLifecycleState::Planned);
    if matches!(
        from,
        SignOnlyLifecycleState::SignedDryRun
            | SignOnlyLifecycleState::Failed
            | SignOnlyLifecycleState::Abandoned
    ) {
        return Err(ServiceError::Conflict(
            "sign-only lifecycle is already terminal".into(),
        ));
    }
    let expected = transition_sign_only_lifecycle(from.clone(), record.event.clone())
        .map_err(|err| ServiceError::Conflict(err.to_string()))?;
    if expected != record.state {
        return Err(ServiceError::Conflict(format!(
            "sign-only lifecycle state mismatch: event {:?} from {:?} yields {:?}, got {:?}",
            record.event, from, expected, record.state
        )));
    }
    match (&record.state, record.signed_order_ref.as_ref()) {
        (SignOnlyLifecycleState::SignedDryRun, Some(value)) if !value.trim().is_empty() => {}
        (SignOnlyLifecycleState::SignedDryRun, _) => {
            return Err(ServiceError::BadRequest(
                "SignedDryRun sign-only lifecycle record requires a non-empty signed_order_ref"
                    .into(),
            ));
        }
        (_, Some(_)) => {
            return Err(ServiceError::BadRequest(
                "signed_order_ref is only allowed for SignedDryRun sign-only lifecycle records"
                    .into(),
            ));
        }
        _ => {}
    }
    Ok(())
}

pub fn verify_snapshot_binding(
    normalized_intent: &NormalizedIntent,
    snapshot: &FeasibilitySnapshot,
) -> Result<(), ServiceError> {
    if snapshot.normalized_intent_id != normalized_intent.normalized_intent_id {
        return Err(ServiceError::Conflict(
            "snapshot does not belong to normalized intent".into(),
        ));
    }
    Ok(())
}

pub fn verify_decision_binding(
    normalized_intent: &NormalizedIntent,
    snapshot: &FeasibilitySnapshot,
    decision: &ConstraintDecision,
) -> Result<(), ServiceError> {
    let expected = evaluate_constraints(normalized_intent, snapshot);
    if &expected != decision {
        return Err(ServiceError::Conflict(
            "decision does not match server recomputation for normalized intent and snapshot"
                .into(),
        ));
    }
    Ok(())
}
