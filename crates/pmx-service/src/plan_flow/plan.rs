use pmx_core::{
    ApprovalReceipt, ConstraintDecision, DecisionStatus, ExecutionPlanSummary, FeasibilitySnapshot,
    HashValue, NormalizedIntent, PlanStatus, canonical_json_sha256,
};
use pmx_store::ExecutionStore;

use crate::{
    CompilePlanByIdCommand, CompilePlanCommand, PlanHashInput, ServiceError,
    verify_decision_binding, verify_snapshot_binding,
};

pub async fn compile_plan<S>(
    store: &S,
    req: CompilePlanCommand,
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
    )
    .await
}

pub async fn compile_plan_by_id<S>(
    store: &S,
    req: CompilePlanByIdCommand,
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
    build_and_save_plan(store, &normalized, &snapshot, &decision, &req.approval).await
}

async fn build_and_save_plan<S>(
    store: &S,
    normalized: &NormalizedIntent,
    snapshot: &FeasibilitySnapshot,
    decision: &ConstraintDecision,
    approval: &ApprovalReceipt,
) -> Result<ExecutionPlanSummary, ServiceError>
where
    S: ExecutionStore + Send + Sync,
{
    let status = if matches!(decision.status, DecisionStatus::Allow) {
        PlanStatus::Ready
    } else {
        PlanStatus::Blocked
    };
    let execution_id = format!("exec-{}", normalized.normalized_intent_id);
    let mut plan = ExecutionPlanSummary {
        execution_id,
        account_id: normalized.account_id.clone(),
        normalized_intent_id: normalized.normalized_intent_id.clone(),
        snapshot_id: snapshot.snapshot_id.clone(),
        decision_id: decision.decision_id.clone(),
        plan_hash: HashValue("pending".into()),
        status,
        max_exposure: pmx_core::DecimalString("0".into()),
        explanation: vec![
            "v0.15 server-authoritative ID-bound service with admin audit scaffold; live signing/posting remain disabled".into(),
            format!("approval_id={}", approval.approval_id),
            format!("snapshot_id={}", snapshot.snapshot_id),
        ],
    };
    plan.plan_hash = canonical_json_sha256(&PlanHashInput::from(&plan))
        .map_err(|err| ServiceError::Internal(err.to_string()))?;
    store.save_plan_summary(&plan).await?;
    Ok(plan)
}
