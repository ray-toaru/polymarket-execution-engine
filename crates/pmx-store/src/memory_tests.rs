use super::*;
use crate::*;

#[cfg(test)]
async fn seed_test_plan(store: &InMemoryStore, execution_id: &str, account_id: &str) {
    store
        .save_plan_summary(&ExecutionPlanSummary {
            execution_id: execution_id.into(),
            account_id: pmx_core::AccountId(account_id.into()),
            normalized_intent_id: format!("norm-{execution_id}"),
            snapshot_id: format!("snap-{execution_id}"),
            decision_id: format!("decision-{execution_id}"),
            plan_hash: pmx_core::HashValue(format!("hash-{execution_id}")),
            status: pmx_core::PlanStatus::Ready,
            max_exposure: pmx_core::DecimalString("0".into()),
            explanation: vec!["test plan for sign-only lifecycle FK parity".into()],
        })
        .await
        .expect("seed execution plan");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DEFAULT_RUNTIME_OBSERVATION_TTL_SECONDS, advisory_lock_key, submit_status_str};
    use chrono::Duration;
    use pmx_core::SubmitStatus;

    #[test]
    fn idempotency_identity_is_documented_in_trait() {
        let action = IdempotencyAction::Proceed {
            submit_attempt: 1,
            owner_token: "owner".into(),
        };
        assert_eq!(
            action,
            IdempotencyAction::Proceed {
                submit_attempt: 1,
                owner_token: "owner".into(),
            }
        );
    }

    #[test]
    fn advisory_lock_key_is_deterministic_and_scoped() {
        let a = advisory_lock_key("submit", "acct-1", "exec-1");
        let b = advisory_lock_key("submit", "acct-1", "exec-1");
        let c = advisory_lock_key("submit", "acct-1", "exec-2");
        let d = advisory_lock_key("reservation", "acct-1", "exec-1");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn maps_plan_status_for_db() {
        let status = submit_status_str(&SubmitStatus::RemoteUnknown);
        assert_eq!(status, "REMOTE_UNKNOWN");
    }

    #[tokio::test]
    async fn in_memory_same_request_without_response_is_in_progress() {
        let store = InMemoryStore::default();
        let first = store
            .begin_submit_attempt("acct", "exec", "idem", "req")
            .await
            .expect("first begin");
        assert!(matches!(first, IdempotencyAction::Proceed { .. }));
        let second = store
            .begin_submit_attempt("acct", "exec", "idem", "req")
            .await
            .expect("second begin");
        assert!(matches!(second, IdempotencyAction::InProgress { .. }));
    }

    #[tokio::test]
    async fn runtime_worker_observations_degrade_loaded_runtime_state() {
        let store = InMemoryStore::default();
        store.set_runtime_state_for_test(
            "acct-runtime-observed",
            "cond-runtime-observed",
            None,
            RuntimeStateSummary {
                geoblock_status: GeoblockStatus::Allowed,
                worker_status: WorkerStatus::Healthy,
                collateral_profile_status: CollateralProfileStatus::DefaultResolved,
                kill_switch_enabled: false,
                required_capabilities: vec!["heartbeat".into()],
            },
        );
        store
            .record_runtime_worker_observation(&RuntimeWorkerObservation {
                account_id: "acct-runtime-observed".into(),
                capability: "heartbeat-lease".into(),
                worker_kind: "HeartbeatLease".into(),
                status: "STALE".into(),
                should_fail_closed: true,
                reason: "lease expired".into(),
                observed_at: None,
            })
            .await
            .expect("record observation");
        let state = store
            .load_runtime_state(&RuntimeStateQuery {
                account_id: "acct-runtime-observed".into(),
                condition_id: "cond-runtime-observed".into(),
                collateral_profile_id: None,
                required_capabilities: vec!["heartbeat".into()],
            })
            .await
            .expect("load runtime state");
        assert_eq!(state.worker_status, WorkerStatus::Stale);
        assert!(
            state
                .required_capabilities
                .contains(&"heartbeat-lease".into())
        );
    }

    #[tokio::test]
    async fn stale_runtime_worker_observations_are_ignored() {
        let store = InMemoryStore::default();
        store.set_runtime_state_for_test(
            "acct-runtime-stale-observation",
            "cond-runtime-stale-observation",
            None,
            RuntimeStateSummary {
                geoblock_status: GeoblockStatus::Allowed,
                worker_status: WorkerStatus::Healthy,
                collateral_profile_status: CollateralProfileStatus::DefaultResolved,
                kill_switch_enabled: false,
                required_capabilities: vec!["heartbeat".into()],
            },
        );
        store
            .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                worker_id: "worker-runtime-stale-observation".into(),
                role: "Heartbeat".into(),
                capability: "heartbeat".into(),
                status: "HEALTHY".into(),
                last_heartbeat_at: Utc::now(),
                last_error: None,
            })
            .await
            .expect("record heartbeat");
        store
            .record_runtime_worker_observation(&RuntimeWorkerObservation {
                account_id: "acct-runtime-stale-observation".into(),
                capability: "heartbeat-lease".into(),
                worker_kind: "HeartbeatLease".into(),
                status: "STALE".into(),
                should_fail_closed: true,
                reason: "old lease expiry".into(),
                observed_at: Some(
                    Utc::now() - Duration::seconds(DEFAULT_RUNTIME_OBSERVATION_TTL_SECONDS + 1),
                ),
            })
            .await
            .expect("record stale observation");
        let state = store
            .load_runtime_state(&RuntimeStateQuery {
                account_id: "acct-runtime-stale-observation".into(),
                condition_id: "cond-runtime-stale-observation".into(),
                collateral_profile_id: None,
                required_capabilities: vec!["heartbeat".into()],
            })
            .await
            .expect("load runtime state");
        assert_eq!(state.worker_status, WorkerStatus::Healthy);
        assert!(
            !state
                .required_capabilities
                .contains(&"heartbeat-lease".into())
        );
    }

    // Async behavior tests are intentionally split into repository-specific tests.
}

#[cfg(test)]
mod admin_audit_tests {
    use super::*;
    use pmx_core::sign_only_lifecycle_records_equivalent;

    #[tokio::test]
    async fn in_memory_admin_audit_records_without_exposing_secrets() {
        let store = InMemoryStore::default();
        store
            .record_admin_audit_event(&AdminAuditEvent {
                audit_id: None,
                principal_subject: "admin-token".into(),
                operation: "KillSwitch".into(),
                request_fingerprint: Some("abc123".into()),
                correlation_id: Some("corr-admin-test".into()),
                result: "ACCEPTED".into(),
                created_at: None,
            })
            .await
            .expect("record audit event");
        let len = store
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .admin_audit
            .len();
        assert_eq!(len, 1);
    }

    #[tokio::test]
    async fn in_memory_persists_sign_only_lifecycle_records() {
        let store = InMemoryStore::default();
        let execution_id = pmx_core::ExecutionId("exec-sign-only".into());
        let account_id = pmx_core::AccountId("acct-sign-only".into());
        seed_test_plan(&store, &execution_id.0, &account_id.0).await;
        let records_to_append = [
            SignOnlyLifecycleRecord {
                execution_id: execution_id.clone(),
                account_id: account_id.clone(),
                state: pmx_core::SignOnlyLifecycleState::ReservationPrepared,
                event: pmx_core::SignOnlyLifecycleEventKind::PrepareReservation,
                client_event_id: None,
                signed_order_ref: None,
                no_remote_side_effect: true,
                event_id: None,
                created_at: None,
            },
            SignOnlyLifecycleRecord {
                execution_id: execution_id.clone(),
                account_id: account_id.clone(),
                state: pmx_core::SignOnlyLifecycleState::SigningRequested,
                event: pmx_core::SignOnlyLifecycleEventKind::RequestSigning,
                client_event_id: None,
                signed_order_ref: None,
                no_remote_side_effect: true,
                event_id: None,
                created_at: None,
            },
            SignOnlyLifecycleRecord {
                execution_id: execution_id.clone(),
                account_id: account_id.clone(),
                state: pmx_core::SignOnlyLifecycleState::SignedDryRun,
                event: pmx_core::SignOnlyLifecycleEventKind::SignedWithoutPost,
                client_event_id: None,
                signed_order_ref: Some("sign-only:redacted-ref".into()),
                no_remote_side_effect: true,
                event_id: None,
                created_at: None,
            },
        ];
        for record in &records_to_append {
            store
                .record_sign_only_lifecycle_event(record)
                .await
                .expect("record sign-only lifecycle");
        }
        let records = store
            .list_sign_only_lifecycle_events(&SignOnlyLifecycleQuery {
                execution_id: "exec-sign-only".into(),
                limit: 100,
                before_event_id: None,
            })
            .await
            .expect("list sign-only lifecycle");
        assert_eq!(records.len(), 3);
        assert!(records.iter().all(|record| record.event_id.is_some()));
        assert!(records.iter().all(|record| record.created_at.is_some()));
        assert!(sign_only_lifecycle_records_equivalent(
            records.last().unwrap(),
            records_to_append.last().unwrap()
        ));
    }

    #[tokio::test]
    async fn in_memory_sign_only_replay_is_idempotent() {
        let store = InMemoryStore::default();
        seed_test_plan(&store, "exec-sign-only-replay", "acct-sign-only-replay").await;
        let record = SignOnlyLifecycleRecord {
            execution_id: pmx_core::ExecutionId("exec-sign-only-replay".into()),
            account_id: pmx_core::AccountId("acct-sign-only-replay".into()),
            state: pmx_core::SignOnlyLifecycleState::ReservationPrepared,
            event: pmx_core::SignOnlyLifecycleEventKind::PrepareReservation,
            client_event_id: None,
            signed_order_ref: None,
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        };
        store
            .record_sign_only_lifecycle_event(&record)
            .await
            .expect("record sign-only lifecycle");
        store
            .record_sign_only_lifecycle_event(&record)
            .await
            .expect("replay sign-only lifecycle");
        let records = store
            .list_sign_only_lifecycle_events(&SignOnlyLifecycleQuery {
                execution_id: "exec-sign-only-replay".into(),
                limit: 100,
                before_event_id: None,
            })
            .await
            .expect("list sign-only lifecycle");
        assert_eq!(records.len(), 1);
    }

    #[tokio::test]
    async fn in_memory_sign_only_client_event_id_replays_and_rejects_mismatch() {
        let store = InMemoryStore::default();
        seed_test_plan(
            &store,
            "exec-sign-only-client-event",
            "acct-sign-only-client-event",
        )
        .await;
        let record = SignOnlyLifecycleRecord {
            execution_id: pmx_core::ExecutionId("exec-sign-only-client-event".into()),
            account_id: pmx_core::AccountId("acct-sign-only-client-event".into()),
            state: pmx_core::SignOnlyLifecycleState::ReservationPrepared,
            event: pmx_core::SignOnlyLifecycleEventKind::PrepareReservation,
            client_event_id: Some("client-event-1".into()),
            signed_order_ref: None,
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        };
        store
            .record_sign_only_lifecycle_event(&record)
            .await
            .expect("record sign-only lifecycle");
        store
            .record_sign_only_lifecycle_event(&record)
            .await
            .expect("replay client_event_id");
        let mut mismatched = record.clone();
        mismatched.event = pmx_core::SignOnlyLifecycleEventKind::Abandon;
        assert!(matches!(
            store.record_sign_only_lifecycle_event(&mismatched).await,
            Err(StoreError::Conflict(_))
        ));
        let records = store
            .list_sign_only_lifecycle_events(&SignOnlyLifecycleQuery {
                execution_id: "exec-sign-only-client-event".into(),
                limit: 100,
                before_event_id: None,
            })
            .await
            .expect("list sign-only lifecycle");
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].client_event_id.as_deref(),
            Some("client-event-1")
        );
    }

    #[tokio::test]
    async fn in_memory_rejects_sign_only_for_unknown_execution() {
        let store = InMemoryStore::default();
        let record = SignOnlyLifecycleRecord {
            execution_id: pmx_core::ExecutionId("missing-exec".into()),
            account_id: pmx_core::AccountId("acct-missing-exec".into()),
            state: pmx_core::SignOnlyLifecycleState::ReservationPrepared,
            event: pmx_core::SignOnlyLifecycleEventKind::PrepareReservation,
            client_event_id: Some("missing-exec-event".into()),
            signed_order_ref: None,
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        };
        assert!(matches!(
            store.record_sign_only_lifecycle_event(&record).await,
            Err(StoreError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn in_memory_rejects_sign_only_remote_side_effect_records() {
        let store = InMemoryStore::default();
        seed_test_plan(&store, "exec-sign-only", "acct-sign-only").await;
        let record = SignOnlyLifecycleRecord {
            execution_id: pmx_core::ExecutionId("exec-sign-only".into()),
            account_id: pmx_core::AccountId("acct-sign-only".into()),
            state: pmx_core::SignOnlyLifecycleState::SignedDryRun,
            event: pmx_core::SignOnlyLifecycleEventKind::SignedWithoutPost,
            client_event_id: None,
            signed_order_ref: Some("sign-only:redacted-ref".into()),
            no_remote_side_effect: false,
            event_id: None,
            created_at: None,
        };
        assert!(
            store
                .record_sign_only_lifecycle_event(&record)
                .await
                .is_err()
        );
    }
}

#[cfg(test)]
mod runtime_worker_health_tests_v23 {
    use super::*;
    use crate::runtime_observation_ttl_seconds;
    use chrono::Duration;
    use pmx_core::{CollateralProfileStatus, GeoblockStatus, RuntimeStateSummary, WorkerStatus};

    #[tokio::test]
    async fn in_memory_worker_heartbeat_informs_runtime_state() {
        let store = InMemoryStore::default();
        store.set_runtime_state_for_test(
            "acct-heartbeat",
            "cond-heartbeat",
            None,
            RuntimeStateSummary {
                geoblock_status: GeoblockStatus::Allowed,
                worker_status: WorkerStatus::Unknown,
                collateral_profile_status: CollateralProfileStatus::DefaultResolved,
                kill_switch_enabled: false,
                required_capabilities: vec!["heartbeat".into()],
            },
        );
        store
            .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                worker_id: "worker-heartbeat-1".into(),
                role: "Heartbeat".into(),
                capability: "heartbeat".into(),
                status: "HEALTHY".into(),
                last_heartbeat_at: Utc::now(),
                last_error: None,
            })
            .await
            .expect("record heartbeat");
        let state = store
            .load_runtime_state(&RuntimeStateQuery {
                account_id: "acct-heartbeat".into(),
                condition_id: "cond-heartbeat".into(),
                collateral_profile_id: None,
                required_capabilities: vec!["heartbeat".into()],
            })
            .await
            .expect("runtime state");
        assert_eq!(state.worker_status, WorkerStatus::Healthy);
    }

    #[tokio::test]
    async fn stale_in_memory_worker_heartbeat_fails_closed() {
        let store = InMemoryStore::default();
        store.set_runtime_state_for_test(
            "acct-heartbeat-stale",
            "cond-heartbeat-stale",
            None,
            RuntimeStateSummary {
                geoblock_status: GeoblockStatus::Allowed,
                worker_status: WorkerStatus::Healthy,
                collateral_profile_status: CollateralProfileStatus::DefaultResolved,
                kill_switch_enabled: false,
                required_capabilities: vec!["heartbeat".into()],
            },
        );
        store
            .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                worker_id: "worker-heartbeat-stale".into(),
                role: "Heartbeat".into(),
                capability: "heartbeat".into(),
                status: "HEALTHY".into(),
                last_heartbeat_at: Utc::now()
                    - Duration::seconds(runtime_observation_ttl_seconds() + 1),
                last_error: Some("missed heartbeat".into()),
            })
            .await
            .expect("record heartbeat");
        let state = store
            .load_runtime_state(&RuntimeStateQuery {
                account_id: "acct-heartbeat-stale".into(),
                condition_id: "cond-heartbeat-stale".into(),
                collateral_profile_id: None,
                required_capabilities: vec!["heartbeat".into()],
            })
            .await
            .expect("runtime state");
        assert_eq!(state.worker_status, WorkerStatus::Stale);
    }

    #[tokio::test]
    async fn in_memory_lists_runtime_worker_status() {
        let store = InMemoryStore::default();
        let observed_at = Utc::now();
        store
            .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                worker_id: "worker-status-query".into(),
                role: "Heartbeat".into(),
                capability: "heartbeat".into(),
                status: "HEALTHY".into(),
                last_heartbeat_at: observed_at,
                last_error: None,
            })
            .await
            .expect("record heartbeat");
        store
            .record_runtime_worker_observation(&RuntimeWorkerObservation {
                account_id: "acct-status-query".into(),
                capability: "heartbeat-lease".into(),
                worker_kind: "HeartbeatLease".into(),
                status: "STALE".into(),
                should_fail_closed: true,
                reason: "lease expired".into(),
                observed_at: Some(observed_at),
            })
            .await
            .expect("record observation");
        let report = store
            .list_runtime_worker_status(&RuntimeWorkerStatusQuery {
                account_id: "acct-status-query".into(),
                limit: 100,
                before_observed_at: None,
            })
            .await
            .expect("list runtime worker status");
        assert_eq!(report.heartbeats.len(), 1);
        assert_eq!(report.heartbeats[0].worker_id, "worker-status-query");
        assert_eq!(report.observations.len(), 1);
        assert_eq!(report.observations[0].capability, "heartbeat-lease");
        assert!(report.observations[0].should_fail_closed);
    }
}

#[cfg(test)]
mod order_lifecycle_store_tests_v23 {
    use super::*;
    use pmx_core::{OrderEventKind, OrderLifecycleState};

    fn test_order(order_id: &str) -> OrderLifecycleRecord {
        OrderLifecycleRecord {
            order_id: order_id.into(),
            execution_id: format!("exec-{order_id}"),
            account_id: "acct-order-life".into(),
            condition_id: "cond-order-life".into(),
            token_id: "token-order-life".into(),
            side: "BUY".into(),
            lifecycle_state: OrderLifecycleState::Posted,
            remote_order_id: Some(format!("remote-{order_id}")),
            remote_state: Some("OPEN".into()),
            created_at: None,
            updated_at: None,
        }
    }

    #[tokio::test]
    async fn in_memory_order_lifecycle_records_cancel_requested() {
        let store = InMemoryStore::default();
        store
            .upsert_order_lifecycle(&test_order("order-life-1"))
            .await
            .expect("upsert order");
        let updated = store
            .record_order_lifecycle_event(&OrderLifecycleEventRecord {
                event_id: None,
                order_id: "order-life-1".into(),
                event: OrderEventKind::CancelRequested,
                event_source: "pmx-store-test".into(),
                correlation_id: Some("corr-order-life-1".into()),
                payload: serde_json::json!({"no_remote_side_effect": true}),
                created_at: None,
            })
            .await
            .expect("record event");
        assert_eq!(
            updated.lifecycle_state,
            OrderLifecycleState::CancelRequested
        );
        let events = store
            .list_order_lifecycle_events(&OrderLifecycleEventQuery {
                order_id: "order-life-1".into(),
                limit: 10,
                before_event_id: None,
            })
            .await
            .expect("list events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, OrderEventKind::CancelRequested);
        assert_eq!(
            events[0].correlation_id.as_deref(),
            Some("corr-order-life-1")
        );
        assert!(events[0].event_id.is_some());
    }

    #[tokio::test]
    async fn in_memory_order_lifecycle_rejects_invalid_transition() {
        let store = InMemoryStore::default();
        store
            .upsert_order_lifecycle(&test_order("order-life-invalid"))
            .await
            .expect("upsert order");
        let err = store
            .record_order_lifecycle_event(&OrderLifecycleEventRecord {
                event_id: None,
                order_id: "order-life-invalid".into(),
                event: OrderEventKind::CancelConfirmed,
                event_source: "pmx-store-test".into(),
                correlation_id: None,
                payload: serde_json::json!({}),
                created_at: None,
            })
            .await
            .expect_err("invalid transition");
        assert!(matches!(err, StoreError::Conflict(_)));
    }

    #[tokio::test]
    async fn in_memory_lists_reconcile_backlog_orders() {
        let store = InMemoryStore::default();
        let mut remote_unknown = test_order("order-reconcile-backlog-1");
        remote_unknown.lifecycle_state = OrderLifecycleState::RemoteUnknown;
        let mut partial_remote_unknown = test_order("order-reconcile-backlog-2");
        partial_remote_unknown.lifecycle_state = OrderLifecycleState::PartialRemoteUnknown;
        let posted = test_order("order-reconcile-backlog-posted");
        for order in [&remote_unknown, &partial_remote_unknown, &posted] {
            store
                .upsert_order_lifecycle(order)
                .await
                .expect("upsert order");
        }
        let backlog = store
            .list_reconcile_backlog_orders(&OrderReconcileBacklogQuery {
                account_id: "acct-order-life".into(),
                limit: 100,
            })
            .await
            .expect("list reconcile backlog");
        let order_ids: Vec<_> = backlog
            .iter()
            .map(|order| order.order_id.as_str())
            .collect();
        assert_eq!(order_ids.len(), 2);
        assert!(order_ids.contains(&"order-reconcile-backlog-1"));
        assert!(order_ids.contains(&"order-reconcile-backlog-2"));
    }
}
