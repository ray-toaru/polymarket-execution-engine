use super::*;
use crate::*;
use chrono::Utc;
use pmx_core::sign_only_lifecycle_records_equivalent;
use std::time::{SystemTime, UNIX_EPOCH};

async fn test_store() -> Option<PostgresStore> {
    let Ok(url) = std::env::var("PMX_TEST_DATABASE_URL") else {
        eprintln!("PMX_TEST_DATABASE_URL not set; skipping PostgreSQL repository test");
        return None;
    };
    let store = PostgresStore::connect(url).await.ok()?;
    store.apply_schema().await.expect("apply PostgreSQL schema");
    Some(store)
}

fn unique(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    format!("{prefix}-{nanos}")
}

async fn seed_execution_plan(store: &PostgresStore, account_id: &str, execution_id: &str) {
    let client = store.client().await.expect("test postgres client");
    let norm = unique("norm");
    let snap = unique("snap");
    let dec = unique("decision");
    let plan_hash = unique("plan-hash");
    client
        .execute(
            "INSERT INTO normalized_intents (normalized_intent_id, intent_hash, account_id, payload) \
             VALUES ($1, $2, $3, '{}'::jsonb)",
            &[&norm, &unique("intent-hash"), &account_id],
        )
        .await
        .expect("seed normalized intent");
    client
        .execute(
            "INSERT INTO feasibility_snapshots (snapshot_id, snapshot_hash, normalized_intent_id, payload, captured_at) \
             VALUES ($1, $2, $3, '{}'::jsonb, now())",
            &[&snap, &unique("snapshot-hash"), &norm],
        )
        .await
        .expect("seed snapshot");
    client
        .execute(
            "INSERT INTO constraint_decisions (decision_id, decision_hash, snapshot_id, status, reasons, payload) \
             VALUES ($1, $2, $3, 'ALLOW', '[]'::jsonb, '{}'::jsonb)",
            &[&dec, &unique("decision-hash"), &snap],
        )
        .await
        .expect("seed decision");
    client
        .execute(
            "INSERT INTO execution_plans (execution_id, account_id, normalized_intent_id, snapshot_id, decision_id, plan_hash, status, summary_json) \
             VALUES ($1, $2, $3, $4, $5, $6, 'READY', '{}'::jsonb)",
            &[&execution_id, &account_id, &norm, &snap, &dec, &plan_hash],
        )
        .await
        .expect("seed execution plan");
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmx_core::{
        AccountId, CollateralProfileStatus, DecimalString, ExecutionId, GeoblockStatus,
        OrderReservation, QuantityBound, ReservationState, SignOnlyLifecycleRecord, SubmitReceipt,
        SubmitStatus, WorkerStatus,
    };
    use tokio::task::JoinSet;

    #[tokio::test]
    async fn postgres_records_schema_migrations() {
        let Some(store) = test_store().await else {
            return;
        };
        let migrations = store
            .applied_schema_migrations()
            .await
            .expect("schema migration rows");
        assert!(
            migrations
                .iter()
                .any(|(version, checksum)| version == "0001_initial" && checksum.len() == 64)
        );
        assert!(migrations.iter().any(|(version, checksum)| {
            version == "0002_migration_framework" && checksum.len() == 64
        }));
        assert!(
            migrations
                .iter()
                .any(|(version, checksum)| version == "0003_order_event_trace"
                    && checksum.len() == 64)
        );
    }

    #[tokio::test]
    async fn postgres_records_admin_audit_event() {
        let Some(store) = test_store().await else {
            return;
        };
        let principal = unique("principal");
        store
            .record_admin_audit_event(&AdminAuditEvent {
                audit_id: None,
                principal_subject: principal.clone(),
                operation: "KillSwitch".into(),
                request_fingerprint: Some(unique("request-fp")),
                correlation_id: Some(unique("corr")),
                result: "ACCEPTED".into(),
                created_at: None,
            })
            .await
            .expect("record audit event");
        let client = store.client().await.expect("test postgres client");
        let row = client
            .query_one(
                "SELECT COUNT(*)::bigint FROM admin_audit_events WHERE principal_subject = $1",
                &[&principal],
            )
            .await
            .expect("count audit events");
        let count: i64 = row.get(0);
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn postgres_admin_audit_paginates_and_filters_by_cursor() {
        let Some(store) = test_store().await else {
            return;
        };
        let principal = unique("principal-page");
        let corr_1 = unique("corr-audit-page-1");
        let corr_2 = unique("corr-audit-page-2");
        let corr_3 = unique("corr-audit-page-3");
        for (operation, correlation_id, result) in [
            ("KillSwitch", corr_1.clone(), "ACCEPTED"),
            ("RuntimeOverride", corr_2.clone(), "DENIED"),
            ("KillSwitch", corr_3.clone(), "ACCEPTED"),
        ] {
            store
                .record_admin_audit_event(&AdminAuditEvent {
                    audit_id: None,
                    principal_subject: principal.clone(),
                    operation: operation.into(),
                    request_fingerprint: Some(unique("request-fp-page")),
                    correlation_id: Some(correlation_id),
                    result: result.into(),
                    created_at: None,
                })
                .await
                .expect("record audit page event");
        }

        let first_page = store
            .list_admin_audit_events(&AdminAuditQuery {
                limit: 2,
                principal_subject: Some(principal.clone()),
                ..AdminAuditQuery::default()
            })
            .await
            .expect("first page");
        assert_eq!(first_page.len(), 2);
        assert_eq!(
            first_page
                .iter()
                .map(|event| event.correlation_id.clone())
                .collect::<Vec<_>>(),
            vec![Some(corr_2.clone()), Some(corr_3.clone())]
        );

        let older_page = store
            .list_admin_audit_events(&AdminAuditQuery {
                limit: 2,
                before_audit_id: first_page[0].audit_id,
                principal_subject: Some(principal.clone()),
                ..AdminAuditQuery::default()
            })
            .await
            .expect("older page");
        assert_eq!(older_page.len(), 1);
        assert_eq!(older_page[0].correlation_id, Some(corr_1));

        let filtered = store
            .list_admin_audit_events(&AdminAuditQuery {
                limit: 10,
                operation: Some("KillSwitch".into()),
                result: Some("ACCEPTED".into()),
                correlation_id: Some(corr_3.clone()),
                principal_subject: Some(principal),
                ..AdminAuditQuery::default()
            })
            .await
            .expect("filtered page");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].correlation_id, Some(corr_3));
    }

    #[tokio::test]
    async fn same_request_replay_is_persisted() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct");
        let execution = unique("exec");
        super::tests::seed_execution_plan(&store, &account, &execution).await;
        let action = store
            .begin_submit_attempt(&account, &execution, "idem-1", "req-1")
            .await
            .expect("begin idempotency");
        assert_eq!(
            action,
            IdempotencyAction::Proceed {
                submit_attempt: 1,
                owner_token: format!("owner-{account}-{execution}-1"),
            }
        );
        store
            .finish_submit_attempt(
                &account,
                &execution,
                "idem-1",
                "req-1",
                "resp-1",
                r#"{"status":"accepted"}"#,
            )
            .await
            .expect("finish idempotency");
        let replay = store
            .begin_submit_attempt(&account, &execution, "idem-1", "req-1")
            .await
            .expect("replay idempotency");
        assert!(matches!(
            replay,
            IdempotencyAction::ReplayStoredResponse { .. }
        ));
    }

    #[tokio::test]
    async fn fingerprint_mismatch_is_conflict() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct");
        let execution = unique("exec");
        seed_execution_plan(&store, &account, &execution).await;
        store
            .begin_submit_attempt(&account, &execution, "idem-1", "req-1")
            .await
            .expect("begin idempotency");
        let conflict = store
            .begin_submit_attempt(&account, &execution, "idem-1", "req-2")
            .await
            .expect("conflict result");
        assert_eq!(conflict, IdempotencyAction::Conflict);
    }

    #[tokio::test]
    async fn in_progress_replay_does_not_return_proceed() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct");
        let execution = unique("exec");
        seed_execution_plan(&store, &account, &execution).await;
        let first = store
            .begin_submit_attempt(&account, &execution, "idem-progress", "req-progress")
            .await
            .expect("first begin");
        assert!(matches!(first, IdempotencyAction::Proceed { .. }));
        let second = store
            .begin_submit_attempt(&account, &execution, "idem-progress", "req-progress")
            .await
            .expect("second begin");
        assert!(matches!(second, IdempotencyAction::InProgress { .. }));
    }

    #[tokio::test]
    async fn remote_unknown_is_persisted_conservatively() {
        let Some(store) = test_store().await else {
            return;
        };
        let execution = unique("exec");
        let receipt = SubmitReceipt {
            execution_id: execution.clone(),
            receipt_id: unique("receipt"),
            status: SubmitStatus::RemoteUnknown,
            executor_version: "test".into(),
            contract_version: "test".into(),
        };
        store
            .record_submit_receipt(&receipt)
            .await
            .expect("record remote unknown receipt");
        let client = store.client().await.expect("test postgres client");
        let status: String = client
            .query_one(
                "SELECT status FROM submit_receipts WHERE execution_id = $1",
                &[&execution],
            )
            .await
            .expect("query receipt")
            .get(0);
        assert_eq!(status, "REMOTE_UNKNOWN");
    }

    #[tokio::test]
    async fn reservation_double_spend_is_prevented_concurrently() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct");
        let execution = unique("exec");
        seed_execution_plan(&store, &account, &execution).await;
        let reservation = OrderReservation {
            reservation_id: unique("reservation"),
            account_id: AccountId(account.clone()),
            execution_id: ExecutionId(execution.clone()),
            internal_order_id: None,
            quantity_bound: QuantityBound::WorstCaseQuoteNotional(DecimalString("10".into())),
            state: ReservationState::Active,
        };
        let a = store.clone();
        let b = store.clone();
        let r1 = reservation.clone();
        let r2 = reservation;
        let (left, right) = tokio::join!(
            async move { a.save_order_reservation(&r1).await },
            async move { b.save_order_reservation(&r2).await }
        );
        assert!(left.is_ok() || right.is_ok());
        let client = store.client().await.expect("test postgres client");
        let count: i64 = client
            .query_one(
                "SELECT COUNT(*) FROM order_reservations WHERE account_id = $1 AND execution_id = $2",
                &[&account, &execution],
            )
            .await
            .expect("query reservations")
            .get(0);
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn postgres_records_execution_lifecycle_event() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct-life");
        let execution = unique("exec-life");
        seed_execution_plan(&store, &account, &execution).await;
        store
            .record_execution_lifecycle_event(&ExecutionLifecycleEvent {
                event_id: None,
                execution_id: execution.clone(),
                account_id: account.clone(),
                event_type: "SUBMIT_BLOCKED_BEFORE_REMOTE".into(),
                event_source: "pmx-service".into(),
                payload: serde_json::json!({"no_remote_side_effect": true}),
                created_at: None,
            })
            .await
            .expect("record lifecycle event");
        let client = store.client().await.expect("test postgres client");
        let count: i64 = client
            .query_one(
                "SELECT COUNT(*)::bigint FROM execution_lifecycle_events WHERE execution_id = $1 AND event_type = 'SUBMIT_BLOCKED_BEFORE_REMOTE'",
                &[&execution],
            )
            .await
            .expect("count lifecycle events")
            .get(0);
        assert_eq!(count, 1);
    }
    #[tokio::test]
    async fn postgres_loads_runtime_state_from_runtime_tables() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct-runtime");
        let condition = unique("cond-runtime");
        let profile = unique("profile-runtime");
        let client = store.client().await.expect("test postgres client");
        client
            .execute(
                "INSERT INTO runtime_accounts (account_id, status, kill_switch_enabled) VALUES ($1, 'ACTIVE', false)",
                &[&account],
            )
            .await
            .expect("seed runtime account");
        client
            .execute(
                "INSERT INTO runtime_markets (condition_id, status, is_sports) VALUES ($1, 'ACTIVE', false)",
                &[&condition],
            )
            .await
            .expect("seed runtime market");
        client
            .execute(
                "INSERT INTO collateral_profiles (profile_id, status, quote_asset_symbol, quote_asset_address, allowance_target, decimals, profile_version) \
                 VALUES ($1, 'RESOLVED', 'pUSD', '0x0000000000000000000000000000000000000001', '0x0000000000000000000000000000000000000002', 6, 'test')",
                &[&profile],
            )
            .await
            .expect("seed collateral profile");
        for capability in ["heartbeat", "reconcile", "resource-refresh"] {
            let worker_id = unique(&format!("worker-{capability}"));
            let capability_value = capability.to_string();
            client
                .execute(
                    "INSERT INTO worker_health (worker_id, role, capability, status, last_heartbeat_at) \
                     VALUES ($1, 'test', $2, 'HEALTHY', now())",
                    &[&worker_id, &capability_value],
                )
                .await
                .expect("seed worker health");
        }
        let state = store
            .load_runtime_state(&RuntimeStateQuery {
                account_id: account,
                condition_id: condition,
                collateral_profile_id: Some(profile),
                required_capabilities: vec![
                    "heartbeat".into(),
                    "reconcile".into(),
                    "resource-refresh".into(),
                ],
            })
            .await
            .expect("runtime state");
        assert_eq!(state.geoblock_status, GeoblockStatus::Allowed);
        assert_eq!(state.worker_status, WorkerStatus::Healthy);
        assert_eq!(
            state.collateral_profile_status,
            CollateralProfileStatus::Resolved
        );
        assert!(!state.kill_switch_enabled);
    }

    #[tokio::test]
    async fn postgres_persists_sign_only_lifecycle_records() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct-sign-only");
        let execution = unique("exec-sign-only");
        seed_execution_plan(&store, &account, &execution).await;
        let records_to_append = [
            SignOnlyLifecycleRecord {
                execution_id: pmx_core::ExecutionId(execution.clone()),
                account_id: pmx_core::AccountId(account.clone()),
                state: pmx_core::SignOnlyLifecycleState::ReservationPrepared,
                event: pmx_core::SignOnlyLifecycleEventKind::PrepareReservation,
                client_event_id: None,
                signed_order_ref: None,
                no_remote_side_effect: true,
                event_id: None,
                created_at: None,
            },
            SignOnlyLifecycleRecord {
                execution_id: pmx_core::ExecutionId(execution.clone()),
                account_id: pmx_core::AccountId(account.clone()),
                state: pmx_core::SignOnlyLifecycleState::SigningRequested,
                event: pmx_core::SignOnlyLifecycleEventKind::RequestSigning,
                client_event_id: None,
                signed_order_ref: None,
                no_remote_side_effect: true,
                event_id: None,
                created_at: None,
            },
            SignOnlyLifecycleRecord {
                execution_id: pmx_core::ExecutionId(execution.clone()),
                account_id: pmx_core::AccountId(account.clone()),
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
                .expect("record sign-only lifecycle event");
        }
        store
            .record_sign_only_lifecycle_event(records_to_append.last().unwrap())
            .await
            .expect("replay terminal sign-only lifecycle event");
        let records = store
            .list_sign_only_lifecycle_events(&SignOnlyLifecycleQuery {
                execution_id: execution.clone(),
                limit: 100,
                before_event_id: None,
            })
            .await
            .expect("list sign-only lifecycle events");
        assert_eq!(records.len(), 3);
        assert!(records.iter().all(|record| record.event_id.is_some()));
        assert!(records.iter().all(|record| record.created_at.is_some()));
        assert!(sign_only_lifecycle_records_equivalent(
            records.last().unwrap(),
            records_to_append.last().unwrap()
        ));
    }

    #[tokio::test]
    async fn postgres_sign_only_client_event_id_is_idempotent_under_concurrent_replay() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct-sign-only-concurrent");
        let execution = unique("exec-sign-only-concurrent");
        seed_execution_plan(&store, &account, &execution).await;
        let record = SignOnlyLifecycleRecord {
            execution_id: pmx_core::ExecutionId(execution.clone()),
            account_id: pmx_core::AccountId(account.clone()),
            state: pmx_core::SignOnlyLifecycleState::ReservationPrepared,
            event: pmx_core::SignOnlyLifecycleEventKind::PrepareReservation,
            client_event_id: Some(unique("client-event-prepare")),
            signed_order_ref: None,
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        };

        let mut attempts = JoinSet::new();
        for _ in 0..8 {
            let store = store.clone();
            let record = record.clone();
            attempts.spawn(async move { store.record_sign_only_lifecycle_event(&record).await });
        }
        while let Some(result) = attempts.join_next().await {
            result
                .expect("concurrent sign-only lifecycle task")
                .expect("concurrent replay must be idempotent");
        }

        let mut mismatched_replay = record.clone();
        mismatched_replay.event = pmx_core::SignOnlyLifecycleEventKind::Abandon;
        mismatched_replay.state = pmx_core::SignOnlyLifecycleState::Abandoned;
        assert!(matches!(
            store
                .record_sign_only_lifecycle_event(&mismatched_replay)
                .await,
            Err(StoreError::Conflict(_))
        ));

        let signing_requested = SignOnlyLifecycleRecord {
            execution_id: pmx_core::ExecutionId(execution.clone()),
            account_id: pmx_core::AccountId(account.clone()),
            state: pmx_core::SignOnlyLifecycleState::SigningRequested,
            event: pmx_core::SignOnlyLifecycleEventKind::RequestSigning,
            client_event_id: Some(unique("client-event-request-signing")),
            signed_order_ref: None,
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        };
        store
            .record_sign_only_lifecycle_event(&signing_requested)
            .await
            .expect("record signing requested");
        store
            .record_sign_only_lifecycle_event(&SignOnlyLifecycleRecord {
                execution_id: pmx_core::ExecutionId(execution.clone()),
                account_id: pmx_core::AccountId(account.clone()),
                state: pmx_core::SignOnlyLifecycleState::SignedDryRun,
                event: pmx_core::SignOnlyLifecycleEventKind::SignedWithoutPost,
                client_event_id: Some(unique("client-event-signed-dry-run")),
                signed_order_ref: Some("sign-only:redacted-concurrent-ref".into()),
                no_remote_side_effect: true,
                event_id: None,
                created_at: None,
            })
            .await
            .expect("record terminal signed dry run");
        assert!(matches!(
            store
                .record_sign_only_lifecycle_event(&SignOnlyLifecycleRecord {
                    execution_id: pmx_core::ExecutionId(execution.clone()),
                    account_id: pmx_core::AccountId(account.clone()),
                    state: pmx_core::SignOnlyLifecycleState::Abandoned,
                    event: pmx_core::SignOnlyLifecycleEventKind::Abandon,
                    client_event_id: Some(unique("client-event-after-terminal")),
                    signed_order_ref: None,
                    no_remote_side_effect: true,
                    event_id: None,
                    created_at: None,
                })
                .await,
            Err(StoreError::Conflict(_))
        ));

        let records = store
            .list_sign_only_lifecycle_events(&SignOnlyLifecycleQuery {
                execution_id: execution.clone(),
                limit: 100,
                before_event_id: None,
            })
            .await
            .expect("list sign-only lifecycle events");
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].client_event_id, record.client_event_id);
        assert!(records.iter().all(|record| record.no_remote_side_effect));
    }

    #[tokio::test]
    async fn postgres_runtime_worker_observations_degrade_runtime_state() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct-runtime-observed");
        let condition = unique("cond-runtime-observed");
        let profile = unique("profile-runtime-observed");
        let client = store.client().await.expect("test postgres client");
        client
            .execute(
                "INSERT INTO runtime_accounts (account_id, status, kill_switch_enabled) VALUES ($1, 'ACTIVE', false)",
                &[&account],
            )
            .await
            .expect("seed runtime account");
        client
            .execute(
                "INSERT INTO runtime_markets (condition_id, status, is_sports) VALUES ($1, 'ACTIVE', false)",
                &[&condition],
            )
            .await
            .expect("seed runtime market");
        client
            .execute(
                "INSERT INTO collateral_profiles (profile_id, status, quote_asset_symbol, quote_asset_address, allowance_target, decimals, profile_version) \
                 VALUES ($1, 'RESOLVED', 'pUSD', '0x0000000000000000000000000000000000000001', '0x0000000000000000000000000000000000000002', 6, 'test')",
                &[&profile],
            )
            .await
            .expect("seed collateral profile");
        for capability in ["heartbeat", "reconcile", "resource-refresh"] {
            let worker_id = unique(&format!("worker-{capability}"));
            let capability_value = capability.to_string();
            client
                .execute(
                    "INSERT INTO worker_health (worker_id, role, capability, status, last_heartbeat_at) \
                     VALUES ($1, 'test', $2, 'HEALTHY', now())",
                    &[&worker_id, &capability_value],
                )
                .await
                .expect("seed worker health");
        }
        store
            .record_runtime_worker_observation(&RuntimeWorkerObservation {
                account_id: account.clone(),
                capability: "heartbeat-lease".into(),
                worker_kind: "HeartbeatLease".into(),
                status: "STALE".into(),
                should_fail_closed: true,
                reason: "lease expired".into(),
                observed_at: None,
            })
            .await
            .expect("record runtime worker observation");
        let state = store
            .load_runtime_state(&RuntimeStateQuery {
                account_id: account,
                condition_id: condition,
                collateral_profile_id: Some(profile),
                required_capabilities: vec![
                    "heartbeat".into(),
                    "reconcile".into(),
                    "resource-refresh".into(),
                ],
            })
            .await
            .expect("runtime state");
        assert_eq!(state.worker_status, WorkerStatus::Stale);
        assert!(
            state
                .required_capabilities
                .contains(&"heartbeat-lease".into())
        );
    }

    #[tokio::test]
    async fn postgres_records_cancel_reconcile_lifecycle_events() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct-cancel-life");
        let execution = unique("exec-cancel-life");
        seed_execution_plan(&store, &account, &execution).await;
        for event_type in ["CANCEL_REQUESTED_NON_LIVE", "RECONCILE_REQUESTED_NON_LIVE"] {
            store
                .record_execution_lifecycle_event(&ExecutionLifecycleEvent {
                    event_id: None,
                    execution_id: execution.clone(),
                    account_id: account.clone(),
                    event_type: event_type.into(),
                    event_source: "pmx-store-test".into(),
                    payload: serde_json::json!({"no_remote_side_effect": true}),
                    created_at: None,
                })
                .await
                .expect("record lifecycle event");
        }
        let events = store
            .list_execution_lifecycle_events(&ExecutionLifecycleQuery {
                execution_id: execution.clone(),
                limit: 100,
                before_event_id: None,
            })
            .await
            .expect("list lifecycle events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "CANCEL_REQUESTED_NON_LIVE");
        assert_eq!(events[1].event_type, "RECONCILE_REQUESTED_NON_LIVE");
    }

    #[tokio::test]
    async fn postgres_records_runtime_worker_observation() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct-worker-observation");
        store
            .record_runtime_worker_observation(&RuntimeWorkerObservation {
                account_id: account.clone(),
                capability: "heartbeat-lease".into(),
                worker_kind: "HeartbeatLease".into(),
                status: "STALE".into(),
                should_fail_closed: true,
                reason: "lease expired".into(),
                observed_at: None,
            })
            .await
            .expect("record runtime worker observation");
        let client = store.client().await.expect("test postgres client");
        let count: i64 = client
            .query_one(
                "SELECT COUNT(*)::bigint FROM runtime_worker_observations WHERE account_id = $1",
                &[&account],
            )
            .await
            .expect("count runtime worker observations")
            .get(0);
        assert_eq!(count, 1);
    }
}

#[path = "postgres_tests/order_lifecycle.rs"]
mod order_lifecycle;
#[path = "postgres_tests/runtime_worker_health.rs"]
mod runtime_worker_health;
