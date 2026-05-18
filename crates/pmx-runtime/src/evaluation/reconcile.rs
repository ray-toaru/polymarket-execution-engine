use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReconcileBacklogEvaluationInput {
    pub remote_unknown_order_ids: Vec<String>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReconcileBacklogEvaluation {
    pub remote_unknown_orders: u32,
    pub submit_blocked: bool,
    pub reason: String,
}

/// Evaluate reconcile backlog without reading or mutating remote order state.
pub fn evaluate_reconcile_backlog(
    input: ReconcileBacklogEvaluationInput,
) -> ReconcileBacklogEvaluation {
    let remote_unknown_orders = input.remote_unknown_order_ids.len() as u32;
    ReconcileBacklogEvaluation {
        remote_unknown_orders,
        submit_blocked: remote_unknown_orders > 0,
        reason: if remote_unknown_orders == 0 {
            "no remote unknown reconcile backlog".into()
        } else {
            format!("remote_unknown_orders={remote_unknown_orders}")
        },
    }
}
