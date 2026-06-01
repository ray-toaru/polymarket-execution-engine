use pmx_core::{
    BlockReason, ConstraintDecision, DecisionStatus, FeasibilitySnapshot, NormalizedIntent,
    QuantityBound, canonical_json_sha256,
};
use serde::Serialize;

use crate::runtime::collect_runtime_reasons;

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct DecisionHashInput<'a> {
    decision_id: &'a str,
    normalized_intent_id: &'a str,
    snapshot_id: &'a str,
    snapshot_hash: &'a pmx_core::HashValue,
    status: &'a DecisionStatus,
    reasons: &'a [BlockReason],
}

pub fn evaluate_constraints(
    intent: &NormalizedIntent,
    snapshot: &FeasibilitySnapshot,
) -> ConstraintDecision {
    let mut reasons = Vec::new();
    collect_runtime_reasons(&snapshot.runtime_state, &mut reasons);

    if matches!(intent.quantity_bound, QuantityBound::Unsupported(_)) {
        reasons.push(BlockReason::UnsupportedQuantityBound);
    }

    let status = if reasons.is_empty() {
        DecisionStatus::Allow
    } else {
        DecisionStatus::Block
    };
    let decision_id = format!("decision-{}", snapshot.snapshot_id);
    let decision_hash = canonical_json_sha256(&DecisionHashInput {
        decision_id: &decision_id,
        normalized_intent_id: &intent.normalized_intent_id,
        snapshot_id: &snapshot.snapshot_id,
        snapshot_hash: &snapshot.snapshot_hash,
        status: &status,
        reasons: &reasons,
    })
    .expect("decision hash input must be serializable");

    ConstraintDecision {
        decision_id,
        decision_hash,
        correlation_id: None,
        status,
        reasons,
    }
}
