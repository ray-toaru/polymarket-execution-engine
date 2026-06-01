use super::*;

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
    let mut expected = evaluate_constraints(normalized_intent, snapshot);
    expected.correlation_id = decision.correlation_id.clone();
    if &expected != decision {
        return Err(ServiceError::Conflict(
            "decision does not match server recomputation for normalized intent and snapshot"
                .into(),
        ));
    }
    Ok(())
}
