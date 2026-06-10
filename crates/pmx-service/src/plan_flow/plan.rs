use chrono::Utc;
use pmx_core::{
    ApprovalReceipt, ConstraintDecision, DecisionStatus, ExecutionPlanSummary, FeasibilitySnapshot,
    HashValue, NormalizedIntent, PlanStatus, QuantityBound, canonical_json_sha256,
};
use pmx_store::ExecutionStore;

use crate::{
    ApprovalHashInput, CompilePlanByIdCommand, CompilePlanCommand, PlanHashInput, ServiceError,
    verify_decision_binding, verify_snapshot_binding,
};

struct PlanBuildContext<'a> {
    normalized: &'a NormalizedIntent,
    snapshot: &'a FeasibilitySnapshot,
    decision: &'a ConstraintDecision,
    approval: &'a ApprovalReceipt,
    executor_version: &'a str,
    contract_version: &'a str,
    correlation_id: Option<String>,
}

pub async fn compile_plan<S>(
    store: &S,
    req: CompilePlanCommand,
    executor_version: &str,
    contract_version: &str,
) -> Result<ExecutionPlanSummary, ServiceError>
where
    S: ExecutionStore + Send + Sync,
{
    let CompilePlanCommand {
        normalized_intent,
        snapshot,
        decision,
        approval,
        correlation_id,
    } = req;
    verify_snapshot_binding(&normalized_intent, &snapshot)?;
    verify_decision_binding(&normalized_intent, &snapshot, &decision)?;
    store.save_normalized_intent(&normalized_intent).await?;
    store.save_snapshot(&snapshot).await?;
    store.save_decision(&decision).await?;
    build_and_save_plan(
        store,
        PlanBuildContext {
            normalized: &normalized_intent,
            snapshot: &snapshot,
            decision: &decision,
            approval: &approval,
            executor_version,
            contract_version,
            correlation_id,
        },
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
        PlanBuildContext {
            normalized: &normalized,
            snapshot: &snapshot,
            decision: &decision,
            approval: &req.approval,
            executor_version,
            contract_version,
            correlation_id: req.correlation_id,
        },
    )
    .await
}

async fn build_and_save_plan<S>(
    store: &S,
    context: PlanBuildContext<'_>,
) -> Result<ExecutionPlanSummary, ServiceError>
where
    S: ExecutionStore + Send + Sync,
{
    let PlanBuildContext {
        normalized,
        snapshot,
        decision,
        approval,
        executor_version,
        contract_version,
        correlation_id,
    } = context;
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
        correlation_id: correlation_id
            .or_else(|| decision.correlation_id.clone())
            .or_else(|| snapshot.correlation_id.clone())
            .or_else(|| normalized.correlation_id.clone()),
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
    let computed_approval_hash = approval_receipt_hash(approval)?;
    if computed_approval_hash != approval.approval_hash {
        return Err(ServiceError::Conflict(
            "approval_hash does not match canonical approval receipt".into(),
        ));
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

pub fn approval_receipt_hash(approval: &ApprovalReceipt) -> Result<HashValue, ServiceError> {
    canonical_json_sha256(&ApprovalHashInput::from(approval))
        .map_err(|err| ServiceError::Internal(err.to_string()))
}

fn zero_hash() -> HashValue {
    HashValue::from_sha256_hex("0000000000000000000000000000000000000000000000000000000000000000")
        .expect("literal zero hash is valid sha256 hex")
}
