use super::super::*;

#[test]
fn worker_crash_recovery_evaluation_requires_fresh_healthy_required_workers() {
    let observed_at = Utc::now();
    let evaluation = evaluate_worker_crash_recovery(WorkerCrashRecoveryEvaluationInput {
        observed_at,
        stale_after_seconds: 30,
        required_capabilities: vec![
            "heartbeat".into(),
            "reconcile".into(),
            "resource-refresh".into(),
        ],
        observations: vec![
            WorkerCrashRecoveryObservation {
                worker_id: "worker-heartbeat".into(),
                capability: "heartbeat".into(),
                status: HealthLevel::Healthy,
                last_heartbeat_at: Some(observed_at - chrono::Duration::seconds(5)),
                last_error: None,
            },
            WorkerCrashRecoveryObservation {
                worker_id: "worker-reconcile".into(),
                capability: "reconcile".into(),
                status: HealthLevel::Stale,
                last_heartbeat_at: Some(observed_at - chrono::Duration::seconds(5)),
                last_error: Some("restart loop".into()),
            },
        ],
    });
    assert!(!evaluation.recovered);
    assert_eq!(evaluation.failed_workers, vec!["worker-reconcile"]);
    assert_eq!(evaluation.missing_capabilities, vec!["resource-refresh"]);
}

#[test]
fn worker_crash_recovery_evaluation_recovers_after_all_required_workers_are_fresh() {
    let observed_at = Utc::now();
    let evaluation = evaluate_worker_crash_recovery(WorkerCrashRecoveryEvaluationInput {
        observed_at,
        stale_after_seconds: 30,
        required_capabilities: vec!["heartbeat".into()],
        observations: vec![WorkerCrashRecoveryObservation {
            worker_id: "worker-heartbeat".into(),
            capability: "heartbeat".into(),
            status: HealthLevel::Healthy,
            last_heartbeat_at: Some(observed_at - chrono::Duration::seconds(1)),
            last_error: None,
        }],
    });
    assert!(evaluation.recovered);
    assert!(evaluation.missing_capabilities.is_empty());
    assert!(evaluation.stale_workers.is_empty());
    assert!(evaluation.failed_workers.is_empty());
}
