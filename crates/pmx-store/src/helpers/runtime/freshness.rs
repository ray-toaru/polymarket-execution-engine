use super::*;

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

pub(crate) fn runtime_worker_heartbeat_is_fresh(heartbeat: &RuntimeWorkerHeartbeat) -> bool {
    heartbeat.last_heartbeat_at >= Utc::now() - Duration::seconds(runtime_observation_ttl_seconds())
}
