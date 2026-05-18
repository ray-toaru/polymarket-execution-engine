use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::HealthLevel;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeartbeatLeaseCandidate {
    pub worker_id: String,
    pub status: HealthLevel,
    pub last_heartbeat_at: DateTime<Utc>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeartbeatLeaseElectionInput {
    pub instance_id: String,
    pub candidates: Vec<HeartbeatLeaseCandidate>,
    pub observed_at: DateTime<Utc>,
    pub stale_after_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeartbeatLeaseElection {
    pub lease_owner_id: String,
    pub lease_owner_active: bool,
    pub healthy_candidate_count: usize,
    pub fail_closed: bool,
    pub reason: String,
}

/// Elect a single heartbeat lease owner from local worker health observations.
///
/// Election is deterministic: only fresh `Healthy` candidates are eligible, the
/// freshest heartbeat wins, and worker_id breaks ties. Absence of a fresh owner
/// fails closed; it must not produce an allow-like runtime state.
pub fn elect_heartbeat_lease_owner(input: HeartbeatLeaseElectionInput) -> HeartbeatLeaseElection {
    let stale_after_seconds = input.stale_after_seconds.max(0);
    let cutoff = input.observed_at - chrono::Duration::seconds(stale_after_seconds);
    let mut healthy: Vec<_> = input
        .candidates
        .into_iter()
        .filter(|candidate| {
            candidate.status == HealthLevel::Healthy && candidate.last_heartbeat_at >= cutoff
        })
        .collect();
    healthy.sort_by(|left, right| {
        right
            .last_heartbeat_at
            .cmp(&left.last_heartbeat_at)
            .then_with(|| left.worker_id.cmp(&right.worker_id))
    });
    let healthy_candidate_count = healthy.len();
    let Some(owner) = healthy.first() else {
        return HeartbeatLeaseElection {
            lease_owner_id: String::new(),
            lease_owner_active: false,
            healthy_candidate_count,
            fail_closed: true,
            reason: "no fresh healthy heartbeat lease candidate".into(),
        };
    };
    let lease_owner_active = owner.worker_id == input.instance_id;
    HeartbeatLeaseElection {
        lease_owner_id: owner.worker_id.clone(),
        lease_owner_active,
        healthy_candidate_count,
        fail_closed: !lease_owner_active,
        reason: if lease_owner_active {
            "local instance owns heartbeat lease".into()
        } else {
            "another fresh heartbeat lease owner is active".into()
        },
    }
}
