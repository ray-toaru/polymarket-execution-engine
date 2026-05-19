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
    let expected = evaluate_constraints(normalized_intent, snapshot);
    if &expected != decision {
        return Err(ServiceError::Conflict(
            "decision does not match server recomputation for normalized intent and snapshot"
                .into(),
        ));
    }
    Ok(())
}
