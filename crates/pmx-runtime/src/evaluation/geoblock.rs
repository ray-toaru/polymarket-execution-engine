use pmx_core::GeoblockStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeoblockEvaluationInput {
    pub status: GeoblockStatus,
    pub observed_at: chrono::DateTime<chrono::Utc>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeoblockEvaluation {
    pub status: GeoblockStatus,
    pub submit_allowed: bool,
    pub reason: String,
}

/// Evaluate geoblock provider status without remote I/O.
pub fn evaluate_geoblock_status(input: GeoblockEvaluationInput) -> GeoblockEvaluation {
    let submit_allowed = matches!(input.status, GeoblockStatus::Allowed);
    GeoblockEvaluation {
        status: input.status,
        submit_allowed,
        reason: if submit_allowed {
            "geoblock provider allowed".into()
        } else {
            input
                .last_error
                .unwrap_or_else(|| "geoblock provider did not allow submit".into())
        },
    }
}
