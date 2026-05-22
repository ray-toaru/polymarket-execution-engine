use chrono::Utc;
use pmx_core::{
    ApprovalReceipt, ConstraintDecision, DecisionStatus, ExecutionPlanSummary, FeasibilitySnapshot,
    HashValue, NormalizedIntent, PlanStatus, QuantityBound, canonical_json_sha256,
};
use pmx_store::ExecutionStore;

use crate::{
    CompilePlanByIdCommand, CompilePlanCommand, PlanHashInput, ServiceError,
    verify_decision_binding, verify_snapshot_binding,
};

pub async fn compile_plan<S>(
    store: &S,
    req: CompilePlanCommand,
    executor_version: &str,
    contract_version: &str,
) -> Result<ExecutionPlanSummary, ServiceError>
where
    S: ExecutionStore + Send + Sync,
{
    verify_snapshot_binding(&req.normalized_intent, &req.snapshot)?;
    verify_decision_binding(&req.normalized_intent, &req.snapshot, &req.decision)?;
    store.save_normalized_intent(&req.normalized_intent).await?;
    store.save_snapshot(&req.snapshot).await?;
    store.save_decision(&req.decision).await?;
    build_and_save_plan(
        store,
        &req.normalized_intent,
        &req.snapshot,
        &req.decision,
        &req.approval,
        executor_version,
        contract_version,
    )
    .await
}

pub async fn compile_plan_by_id<S>(
    store: &S,
    req: CompilePlanByIdCommand,
    executor_version: &str,
    contract_version: &str,
) -> Result<ExecutionPlanSummary, ServiceError>
where
    S: ExecutionStore + Send + Sync,
{
    let normalized = store
        .load_normalized_intent(&req.normalized_intent_id)
        .await?;
    let snapshot = store.load_snapshot(&req.snapshot_id).await?;
    let decision = store.load_decision(&req.decision_id).await?;
    verify_snapshot_binding(&normalized, &snapshot)?;
    verify_decision_binding(&normalized, &snapshot, &decision)?;
    build_and_save_plan(
        store,
        &normalized,
        &snapshot,
        &decision,
        &req.approval,
        executor_version,
        contract_version,
    )
    .await
}

async fn build_and_save_plan<S>(
    store: &S,
    normalized: &NormalizedIntent,
    snapshot: &FeasibilitySnapshot,
    decision: &ConstraintDecision,
    approval: &ApprovalReceipt,
    executor_version: &str,
    contract_version: &str,
) -> Result<ExecutionPlanSummary, ServiceError>
where
    S: ExecutionStore + Send + Sync,
{
    verify_approval_binding(snapshot, decision, approval)?;
    let status = if matches!(decision.status, DecisionStatus::Allow) {
        PlanStatus::Ready
    } else {
        PlanStatus::Blocked
    };
    let max_exposure = max_quote_exposure(normalized)?;
    let mut plan = ExecutionPlanSummary {
        execution_id: "pending".into(),
        account_id: normalized.account_id.clone(),
        normalized_intent_id: normalized.normalized_intent_id.clone(),
        snapshot_id: snapshot.snapshot_id.clone(),
        snapshot_hash: snapshot.snapshot_hash.clone(),
        decision_id: decision.decision_id.clone(),
        decision_hash: decision.decision_hash.clone(),
        approval_id: approval.approval_id.clone(),
        approval_hash: approval.approval_hash.clone(),
        plan_hash: zero_hash(),
        status,
        condition_id: normalized.market.condition_id.clone(),
        token_id: normalized.token_id.clone(),
        side: normalized.side.clone(),
        quantity_bound: normalized.quantity_bound.clone(),
        limit_price: normalized.limit_price.clone(),
        time_in_force: normalized.time_in_force.clone(),
        collateral_profile_id: normalized.collateral_profile_id.clone(),
        max_exposure,
        executor_version: executor_version.to_owned(),
        contract_version: contract_version.to_owned(),
        explanation: vec![
            "server-authoritative plan bound to approval, snapshot, decision, executor, and contract versions".into(),
            format!("approval_id={}", approval.approval_id),
            format!("approval_scope={:?}", approval.approval_scope),
            format!("snapshot_id={}", snapshot.snapshot_id),
        ],
    };
    plan.plan_hash = canonical_json_sha256(&PlanHashInput::from(&plan))
        .map_err(|err| ServiceError::Internal(err.to_string()))?;
    plan.execution_id = format!("exec-{}", &plan.plan_hash.0[..32]);
    if let Some(bound_plan_hash) = &approval.bound_plan_hash {
        if bound_plan_hash != &plan.plan_hash {
            return Err(ServiceError::Conflict(
                "approval bound_plan_hash does not match compiled plan_hash".into(),
            ));
        }
    }
    store.save_plan_summary(&plan).await?;
    Ok(plan)
}

fn max_quote_exposure(
    normalized: &NormalizedIntent,
) -> Result<pmx_core::DecimalString, ServiceError> {
    match (&normalized.side, &normalized.quantity_bound) {
        (_, QuantityBound::WorstCaseQuoteNotional(value)) => Ok(value.clone()),
        (pmx_core::Side::Buy, QuantityBound::WorstCaseBaseShares(value)) => normalized
            .limit_price
            .checked_mul(value)
            .map_err(|err| ServiceError::Internal(err.to_string())),
        (pmx_core::Side::Sell, QuantityBound::WorstCaseBaseShares(_))
        | (_, QuantityBound::Unsupported(_)) => Ok(pmx_core::DecimalString("0".into())),
    }
}

fn verify_approval_binding(
    snapshot: &FeasibilitySnapshot,
    decision: &ConstraintDecision,
    approval: &ApprovalReceipt,
) -> Result<(), ServiceError> {
    if approval.approval_id.trim().is_empty()
        || approval.approved_by.trim().is_empty()
        || approval.operator_identity_ref.trim().is_empty()
    {
        return Err(ServiceError::Conflict(
            "approval id, approved_by, and operator_identity_ref must be non-empty".into(),
        ));
    }
    if approval.expires_at <= approval.approved_at || approval.expires_at <= Utc::now() {
        return Err(ServiceError::Conflict("approval is expired".into()));
    }
    if approval.bound_snapshot_hash != snapshot.snapshot_hash {
        return Err(ServiceError::Conflict(
            "approval bound_snapshot_hash does not match snapshot".into(),
        ));
    }
    if approval.bound_decision_hash != decision.decision_hash {
        return Err(ServiceError::Conflict(
            "approval bound_decision_hash does not match decision".into(),
        ));
    }
    Ok(())
}

fn zero_hash() -> HashValue {
    HashValue::from_sha256_hex("0000000000000000000000000000000000000000000000000000000000000000")
        .expect("literal zero hash is valid sha256 hex")
}
