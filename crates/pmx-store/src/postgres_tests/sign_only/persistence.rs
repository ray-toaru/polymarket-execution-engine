use super::super::*;
use pmx_core::{SignOnlyLifecycleRecord, sign_only_lifecycle_records_equivalent};

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
