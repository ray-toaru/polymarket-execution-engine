use super::super::InMemoryStore;
use crate::{LiveReadEventQuery, LiveReadEventRecord, LiveReadEventStore, StoreError};

#[tokio::test]
async fn in_memory_live_read_events_are_redacted_read_only_and_queryable() {
    let store = InMemoryStore::default();

    store
        .record_live_read_event(&LiveReadEventRecord {
            event_id: None,
            account_id: pmx_core::AccountId("acct-live-read".into()),
            operation: pmx_core::LiveReadOperation::GetOrder,
            outcome: pmx_core::LiveReadOutcome::RemoteUnknown,
            remote_order_id: Some(pmx_core::RemoteOrderId("remote-live-read-1".into())),
            remote_state: None,
            error_category: Some(pmx_core::LiveReadErrorCategory::RemoteUnknown),
            redacted_error_summary: Some(
                "remote unknown api_secret=[REDACTED] signature=[REDACTED]".into(),
            ),
            no_trading_side_effect: true,
            redacted_fields: pmx_core::live_read_redacted_fields(),
            observed_at: None,
        })
        .await
        .expect("record live-read event");

    store
        .record_live_read_event(&LiveReadEventRecord {
            event_id: None,
            account_id: pmx_core::AccountId("acct-live-read".into()),
            operation: pmx_core::LiveReadOperation::ListOpenOrders,
            outcome: pmx_core::LiveReadOutcome::Observed,
            remote_order_id: None,
            remote_state: Some("OPEN".into()),
            error_category: None,
            redacted_error_summary: None,
            no_trading_side_effect: true,
            redacted_fields: pmx_core::live_read_redacted_fields(),
            observed_at: None,
        })
        .await
        .expect("record second live-read event");

    let events = store
        .list_live_read_events(&LiveReadEventQuery {
            limit: 10,
            account_id: Some(pmx_core::AccountId("acct-live-read".into())),
            operation: Some(pmx_core::LiveReadOperation::GetOrder),
            ..LiveReadEventQuery::default()
        })
        .await
        .expect("list live-read events");

    assert_eq!(events.len(), 1);
    assert!(events[0].event_id.is_some());
    assert!(events[0].observed_at.is_some());
    assert!(events[0].no_trading_side_effect);
    assert_eq!(
        events[0].redacted_error_summary.as_deref(),
        Some("remote unknown api_secret=[REDACTED] signature=[REDACTED]")
    );
    assert!(events[0].redacted_fields.contains(&"api_secret".into()));
    assert!(events[0].redacted_fields.contains(&"signature".into()));
}

#[tokio::test]
async fn in_memory_live_read_events_reject_side_effect_capable_records() {
    let store = InMemoryStore::default();

    let err = store
        .record_live_read_event(&LiveReadEventRecord {
            event_id: None,
            account_id: pmx_core::AccountId("acct-live-read".into()),
            operation: pmx_core::LiveReadOperation::GetOrder,
            outcome: pmx_core::LiveReadOutcome::Blocked,
            remote_order_id: None,
            remote_state: None,
            error_category: None,
            redacted_error_summary: None,
            no_trading_side_effect: false,
            redacted_fields: pmx_core::live_read_redacted_fields(),
            observed_at: None,
        })
        .await
        .expect_err("side-effect-capable live-read event must be rejected");

    assert!(
        matches!(err, StoreError::Conflict(message) if message.contains("no-trading-side-effect"))
    );
}
