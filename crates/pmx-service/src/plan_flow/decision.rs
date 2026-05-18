use pmx_core::ConstraintDecision;
use pmx_policy::evaluate_constraints;
use pmx_store::ExecutionStore;

use crate::{DecisionByIdRequest, DecisionRequest, ServiceError, verify_snapshot_binding};

pub async fn evaluate_decision<S>(
    store: &S,
    req: DecisionRequest,
) -> Result<ConstraintDecision, ServiceError>
where
    S: ExecutionStore + Send + Sync,
{
    verify_snapshot_binding(&req.normalized_intent, &req.snapshot)?;
    store.save_normalized_intent(&req.normalized_intent).await?;
    store.save_snapshot(&req.snapshot).await?;
    let decision = evaluate_constraints(&req.normalized_intent, &req.snapshot);
    store.save_decision(&decision).await?;
    Ok(decision)
}

pub async fn evaluate_decision_by_id<S>(
    store: &S,
    req: DecisionByIdRequest,
) -> Result<ConstraintDecision, ServiceError>
where
    S: ExecutionStore + Send + Sync,
{
    let normalized = store
        .load_normalized_intent(&req.normalized_intent_id)
        .await?;
    let snapshot = store.load_snapshot(&req.snapshot_id).await?;
    evaluate_decision(
        store,
        DecisionRequest {
            normalized_intent: normalized,
            snapshot,
        },
    )
    .await
}
