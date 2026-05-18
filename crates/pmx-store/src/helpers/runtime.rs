use chrono::{Duration, Utc};
use pmx_core::{RuntimeStateSummary, WorkerStatus};

use crate::{RuntimeWorkerHeartbeat, RuntimeWorkerObservation};

pub const DEFAULT_RUNTIME_OBSERVATION_TTL_SECONDS: i64 = 120;
pub const RUNTIME_OBSERVATION_TTL_SECONDS: i64 = DEFAULT_RUNTIME_OBSERVATION_TTL_SECONDS;

/// Runtime observation freshness horizon.
///
/// The default is intentionally conservative and remains configurable because v0.23 has not yet
/// established a validated worker heartbeat cadence. Invalid or non-positive values fail closed
/// back to the default instead of silently extending freshness.
pub fn runtime_observation_ttl_seconds() -> i64 {
    std::env::var("PMX_RUNTIME_OBSERVATION_TTL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0 && *value <= 86_400)
        .unwrap_or(DEFAULT_RUNTIME_OBSERVATION_TTL_SECONDS)
}

pub(crate) fn runtime_observation_is_fresh(observation: &RuntimeWorkerObservation) -> bool {
    observation
        .observed_at
        .map(|observed_at| {
            observed_at >= Utc::now() - Duration::seconds(runtime_observation_ttl_seconds())
        })
        .unwrap_or(true)
}

fn runtime_worker_heartbeat_is_fresh(heartbeat: &RuntimeWorkerHeartbeat) -> bool {
    heartbeat.last_heartbeat_at >= Utc::now() - Duration::seconds(runtime_observation_ttl_seconds())
}

pub(crate) fn worker_status_from_heartbeats(
    heartbeats: &[RuntimeWorkerHeartbeat],
    required_capabilities: &[String],
) -> WorkerStatus {
    if required_capabilities.is_empty() {
        return WorkerStatus::Healthy;
    }
    let mut degraded = false;
    for capability in required_capabilities {
        let Some(heartbeat) = heartbeats
            .iter()
            .filter(|heartbeat| &heartbeat.capability == capability)
            .max_by_key(|heartbeat| heartbeat.last_heartbeat_at)
        else {
            return WorkerStatus::Unknown;
        };
        let normalized = heartbeat.status.trim().to_ascii_uppercase();
        if !runtime_worker_heartbeat_is_fresh(heartbeat)
            || matches!(normalized.as_str(), "STALE" | "ERROR" | "DOWN")
        {
            return WorkerStatus::Stale;
        }
        if normalized == "DEGRADED" {
            degraded = true;
        } else if normalized != "HEALTHY" {
            return WorkerStatus::Unknown;
        }
    }
    if degraded {
        WorkerStatus::Degraded
    } else {
        WorkerStatus::Healthy
    }
}

fn runtime_observation_worker_status(
    observations: &[RuntimeWorkerObservation],
) -> Option<WorkerStatus> {
    if observations.is_empty() {
        return None;
    }
    let mut has_degraded = false;
    let mut has_healthy = false;
    for observation in observations {
        let status = observation
            .status
            .trim()
            .to_ascii_uppercase()
            .replace('-', "_");
        if matches!(status.as_str(), "STALE" | "ERROR" | "DOWN") {
            return Some(WorkerStatus::Stale);
        }
        if matches!(status.as_str(), "UNKNOWN" | "UNOBSERVED") {
            return Some(WorkerStatus::Unknown);
        }
        if observation.should_fail_closed || matches!(status.as_str(), "DEGRADED" | "BLOCKED") {
            has_degraded = true;
        }
        if matches!(status.as_str(), "HEALTHY" | "OK" | "ALLOWED") {
            has_healthy = true;
        }
    }
    if has_degraded {
        Some(WorkerStatus::Degraded)
    } else if has_healthy {
        Some(WorkerStatus::Healthy)
    } else {
        Some(WorkerStatus::Unknown)
    }
}

pub fn apply_runtime_worker_observations(
    mut base: RuntimeStateSummary,
    observations: &[RuntimeWorkerObservation],
) -> RuntimeStateSummary {
    if let Some(observed_status) = runtime_observation_worker_status(observations) {
        base.worker_status = worst_worker_status(base.worker_status, observed_status);
        for observation in observations {
            if !base.required_capabilities.contains(&observation.capability) {
                base.required_capabilities
                    .push(observation.capability.clone());
            }
        }
    }
    base
}

fn worst_worker_status(left: WorkerStatus, right: WorkerStatus) -> WorkerStatus {
    use WorkerStatus::*;
    match (left, right) {
        (Stale, _) | (_, Stale) => Stale,
        (Unknown, _) | (_, Unknown) => Unknown,
        (Degraded, _) | (_, Degraded) => Degraded,
        (Healthy, Healthy) => Healthy,
    }
}
