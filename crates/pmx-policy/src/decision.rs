use pmx_core::{
    BlockReason, ConstraintDecision, DecisionStatus, FeasibilitySnapshot, HashValue,
    NormalizedIntent, QuantityBound,
};

use crate::runtime::collect_runtime_reasons;

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

    ConstraintDecision {
        decision_id: format!("decision-{}", snapshot.snapshot_id),
        decision_hash: HashValue(format!("decision-hash-{}", snapshot.snapshot_hash.0)),
        status,
        reasons,
    }
}
