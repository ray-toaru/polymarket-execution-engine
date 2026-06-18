use super::*;

#[tokio::test]
async fn postgres_live_read_events_are_redacted_read_only_and_queryable() {
    let Some(store) = test_store().await else {
        return;
    };
    let account = pmx_core::AccountId(unique("acct-live-read"));
    let remote_order_id = pmx_core::RemoteOrderId(unique("remote-live-read"));

    store
        .record_live_read_event(&LiveReadEventRecord {
            event_id: None,
            account_id: account.clone(),
            operation: pmx_core::LiveReadOperation::GetOrder,
            outcome: pmx_core::LiveReadOutcome::RemoteUnknown,
            remote_order_id: Some(remote_order_id.clone()),
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
        .expect("record postgres live-read event");

    let events = store
        .list_live_read_events(&LiveReadEventQuery {
            limit: 10,
            account_id: Some(account),
            operation: Some(pmx_core::LiveReadOperation::GetOrder),
            remote_order_id: Some(remote_order_id.clone()),
            ..LiveReadEventQuery::default()
        })
        .await
        .expect("query postgres live-read events");

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].remote_order_id, Some(remote_order_id));
    assert!(events[0].no_trading_side_effect);
    assert_eq!(
        events[0].redacted_error_summary.as_deref(),
        Some("remote unknown api_secret=[REDACTED] signature=[REDACTED]")
    );
}

#[tokio::test]
async fn postgres_live_read_events_reject_side_effect_capable_records() {
    let Some(store) = test_store().await else {
        return;
    };

    let err = store
        .record_live_read_event(&LiveReadEventRecord {
            event_id: None,
            account_id: pmx_core::AccountId(unique("acct-live-read-reject")),
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
