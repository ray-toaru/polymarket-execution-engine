use pmx_core::GeoblockStatus;
use pmx_runtime::{
    HealthLevel, HeartbeatLeaseCandidate, HeartbeatLeaseElectionInput,
    RuntimeWorkerProviderSnapshot, elect_heartbeat_lease_owner,
};
use pmx_store::{
    RuntimeWorkerHealthStore, RuntimeWorkerHeartbeat, RuntimeWorkerObservationStore,
    RuntimeWorkerStatusQuery, RuntimeWorkerStatusStore,
};

use crate::model::*;
use crate::runtime_worker::record_runtime_worker_provider_snapshot;

#[path = "heartbeat/election.rs"]
mod election;

#[path = "heartbeat/store.rs"]
mod store;

pub use election::*;
pub use store::*;

fn heartbeat_health_level(status: &str) -> HealthLevel {
    match status {
        "HEALTHY" | "Healthy" | "healthy" => HealthLevel::Healthy,
        "DEGRADED" | "Degraded" | "degraded" => HealthLevel::Degraded,
        "STALE" | "Stale" | "stale" => HealthLevel::Stale,
        _ => HealthLevel::Unknown,
    }
}
