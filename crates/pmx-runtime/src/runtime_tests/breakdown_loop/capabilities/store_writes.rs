use super::super::super::*;

#[test]
fn runtime_worker_store_writes_are_fail_closed_for_bad_signals() {
    let writes = runtime_worker_store_writes(
        "acct-worker-write",
        &[
            RuntimeSignal::Geoblock {
                status: GeoblockStatus::Unknown,
                last_observed_at: None,
                last_error: Some("geoblock unavailable".into()),
            },
            RuntimeSignal::HeartbeatLease {
                active: false,
                last_observed_at: None,
                last_error: Some("heartbeat lease expired".into()),
            },
            RuntimeSignal::ResourceRefresh {
                fresh: false,
                last_observed_at: None,
                last_error: Some("resource refresh stale".into()),
            },
        ],
    );
    assert_eq!(writes.len(), 3);
    assert!(writes.iter().all(|write| write.should_fail_closed));
    assert!(writes.iter().any(|write| write.capability == "geoblock"));
    assert!(
        writes
            .iter()
            .any(|write| write.capability == "heartbeat-lease")
    );
    assert!(
        writes
            .iter()
            .any(|write| write.capability == "resource-refresh")
    );
}
