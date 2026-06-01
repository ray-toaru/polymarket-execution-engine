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
    let DecisionRequest {
        normalized_intent,
        snapshot,
        correlation_id,
    } = req;
    verify_snapshot_binding(&normalized_intent, &snapshot)?;
    store.save_normalized_intent(&normalized_intent).await?;
    store.save_snapshot(&snapshot).await?;
    let mut decision = evaluate_constraints(&normalized_intent, &snapshot);
    decision.correlation_id = correlation_id
        .or_else(|| snapshot.correlation_id.clone())
        .or_else(|| normalized_intent.correlation_id.clone());
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
            correlation_id: req.correlation_id,
        },
    )
    .await
}
