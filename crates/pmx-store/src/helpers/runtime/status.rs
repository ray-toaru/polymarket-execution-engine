use super::*;

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

pub(crate) fn runtime_observation_worker_status(
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

pub(crate) fn worst_worker_status(left: WorkerStatus, right: WorkerStatus) -> WorkerStatus {
    use WorkerStatus::*;
    match (left, right) {
        (Stale, _) | (_, Stale) => Stale,
        (Unknown, _) | (_, Unknown) => Unknown,
        (Degraded, _) | (_, Degraded) => Degraded,
        (Healthy, Healthy) => Healthy,
    }
}
