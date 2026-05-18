use super::super::*;

#[test]
fn heartbeat_lease_election_selects_fresh_owner_and_fails_closed_for_non_owner() {
    let observed_at = Utc::now();
    let election = elect_heartbeat_lease_owner(HeartbeatLeaseElectionInput {
        instance_id: "worker-b".into(),
        observed_at,
        stale_after_seconds: 30,
        candidates: vec![
            HeartbeatLeaseCandidate {
                worker_id: "worker-a".into(),
                status: HealthLevel::Healthy,
                last_heartbeat_at: observed_at - chrono::Duration::seconds(5),
                last_error: None,
            },
            HeartbeatLeaseCandidate {
                worker_id: "worker-b".into(),
                status: HealthLevel::Healthy,
                last_heartbeat_at: observed_at - chrono::Duration::seconds(10),
                last_error: None,
            },
        ],
    });
    assert_eq!(election.lease_owner_id, "worker-a");
    assert!(!election.lease_owner_active);
    assert!(election.fail_closed);
}

#[test]
fn heartbeat_lease_election_has_no_owner_when_all_candidates_are_stale() {
    let observed_at = Utc::now();
    let election = elect_heartbeat_lease_owner(HeartbeatLeaseElectionInput {
        instance_id: "worker-a".into(),
        observed_at,
        stale_after_seconds: 30,
        candidates: vec![HeartbeatLeaseCandidate {
            worker_id: "worker-a".into(),
            status: HealthLevel::Healthy,
            last_heartbeat_at: observed_at - chrono::Duration::seconds(60),
            last_error: Some("missed heartbeat".into()),
        }],
    });
    assert!(election.lease_owner_id.is_empty());
    assert!(election.fail_closed);
    assert_eq!(election.healthy_candidate_count, 0);
}

#[test]
fn resource_refresh_evaluation_accepts_fresh_healthy_observations() {
    let observed_at = Utc::now();
    let evaluation = evaluate_resource_refresh_freshness(ResourceRefreshEvaluationInput {
        observed_at,
        stale_after_seconds: 30,
        observations: vec![
            ResourceRefreshObservation {
                component: ResourceRefreshComponent::Account,
                resource_id: "acct-1".into(),
                refreshed_at: observed_at - chrono::Duration::seconds(5),
                status: HealthLevel::Healthy,
                last_error: None,
            },
            ResourceRefreshObservation {
                component: ResourceRefreshComponent::Market,
                resource_id: "cond-1".into(),
                refreshed_at: observed_at - chrono::Duration::seconds(10),
                status: HealthLevel::Healthy,
                last_error: None,
            },
            ResourceRefreshObservation {
                component: ResourceRefreshComponent::Collateral,
                resource_id: "collateral-1".into(),
                refreshed_at: observed_at - chrono::Duration::seconds(15),
                status: HealthLevel::Healthy,
                last_error: None,
            },
        ],
    });
    assert!(evaluation.fresh);
    assert!(evaluation.stale_components.is_empty());
    assert!(evaluation.failed_components.is_empty());
    assert!(evaluation.missing_components.is_empty());
}

#[test]
fn resource_refresh_evaluation_fails_closed_for_stale_failed_or_missing_inputs() {
    let observed_at = Utc::now();
    let missing = evaluate_resource_refresh_freshness(ResourceRefreshEvaluationInput {
        observed_at,
        stale_after_seconds: 30,
        observations: vec![],
    });
    assert!(!missing.fresh);
    assert_eq!(missing.reason, "no resource refresh observations");
    assert_eq!(
        missing.missing_components,
        vec!["account", "market", "collateral"]
    );

    let evaluation = evaluate_resource_refresh_freshness(ResourceRefreshEvaluationInput {
        observed_at,
        stale_after_seconds: 30,
        observations: vec![
            ResourceRefreshObservation {
                component: ResourceRefreshComponent::Account,
                resource_id: "acct-1".into(),
                refreshed_at: observed_at - chrono::Duration::seconds(31),
                status: HealthLevel::Healthy,
                last_error: None,
            },
            ResourceRefreshObservation {
                component: ResourceRefreshComponent::Collateral,
                resource_id: "collateral-1".into(),
                refreshed_at: observed_at - chrono::Duration::seconds(1),
                status: HealthLevel::Degraded,
                last_error: Some("balance refresh failed".into()),
            },
        ],
    });
    assert!(!evaluation.fresh);
    assert_eq!(evaluation.stale_components, vec!["account:acct-1"]);
    assert_eq!(
        evaluation.failed_components,
        vec!["collateral:collateral-1"]
    );
    assert_eq!(evaluation.missing_components, vec!["market"]);
}
