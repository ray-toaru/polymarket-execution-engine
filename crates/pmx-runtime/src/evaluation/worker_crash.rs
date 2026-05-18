use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::HealthLevel;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkerCrashRecoveryObservation {
    pub worker_id: String,
    pub capability: String,
    pub status: HealthLevel,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkerCrashRecoveryEvaluationInput {
    pub observations: Vec<WorkerCrashRecoveryObservation>,
    pub required_capabilities: Vec<String>,
    pub observed_at: DateTime<Utc>,
    pub stale_after_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkerCrashRecoveryEvaluation {
    pub recovered: bool,
    pub missing_capabilities: Vec<String>,
    pub stale_workers: Vec<String>,
    pub failed_workers: Vec<String>,
    pub reason: String,
}

/// Evaluate whether required runtime workers have recovered after crash/restart.
pub fn evaluate_worker_crash_recovery(
    input: WorkerCrashRecoveryEvaluationInput,
) -> WorkerCrashRecoveryEvaluation {
    let stale_after_seconds = input.stale_after_seconds.max(0);
    let cutoff = input.observed_at - chrono::Duration::seconds(stale_after_seconds);
    let mut missing_capabilities = Vec::new();
    let mut stale_workers = Vec::new();
    let mut failed_workers = Vec::new();

    for capability in &input.required_capabilities {
        let Some(observation) = input
            .observations
            .iter()
            .filter(|observation| &observation.capability == capability)
            .max_by_key(|observation| observation.last_heartbeat_at)
        else {
            missing_capabilities.push(capability.clone());
            continue;
        };
        if observation.status != HealthLevel::Healthy {
            failed_workers.push(observation.worker_id.clone());
            continue;
        }
        if observation
            .last_heartbeat_at
            .map(|last_heartbeat_at| last_heartbeat_at < cutoff)
            .unwrap_or(true)
        {
            stale_workers.push(observation.worker_id.clone());
        }
    }

    let recovered =
        missing_capabilities.is_empty() && stale_workers.is_empty() && failed_workers.is_empty();
    WorkerCrashRecoveryEvaluation {
        recovered,
        missing_capabilities,
        stale_workers,
        failed_workers,
        reason: if recovered {
            "all required workers have fresh healthy heartbeats".into()
        } else {
            "required worker missing, stale, or failed after crash recovery".into()
        },
    }
}
