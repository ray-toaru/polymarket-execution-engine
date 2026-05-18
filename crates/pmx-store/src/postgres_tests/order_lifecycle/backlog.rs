use super::super::*;
use chrono::Utc;
use pmx_core::OrderLifecycleState;

#[tokio::test]
async fn postgres_lists_reconcile_backlog_orders() {
    let Some(store) = test_store().await else {
        return;
    };
    let suffix = Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let account = format!("acct-reconcile-backlog-{suffix}");
    let execution = format!("exec-reconcile-backlog-{suffix}");
    seed_execution_plan(&store, &account, &execution).await;
    for (order_id, lifecycle_state) in [
        (
            format!("order-reconcile-backlog-remote-{suffix}"),
            OrderLifecycleState::RemoteUnknown,
        ),
        (
            format!("order-reconcile-backlog-partial-{suffix}"),
            OrderLifecycleState::PartialRemoteUnknown,
        ),
        (
            format!("order-reconcile-backlog-posted-{suffix}"),
            OrderLifecycleState::Posted,
        ),
    ] {
        store
            .upsert_order_lifecycle(&OrderLifecycleRecord {
                order_id: order_id.clone(),
                execution_id: execution.clone(),
                account_id: account.clone(),
                condition_id: "cond-reconcile-backlog".into(),
                token_id: "token-reconcile-backlog".into(),
                side: "BUY".into(),
                lifecycle_state,
                remote_order_id: Some(format!("remote-{order_id}")),
                remote_state: Some("OPEN".into()),
                created_at: None,
                updated_at: None,
            })
            .await
            .expect("upsert order");
    }
    let backlog = store
        .list_reconcile_backlog_orders(&OrderReconcileBacklogQuery {
            account_id: account,
            limit: 100,
        })
        .await
        .expect("list reconcile backlog");
    assert_eq!(backlog.len(), 2);
    assert!(backlog.iter().all(|order| matches!(
        order.lifecycle_state,
        OrderLifecycleState::RemoteUnknown | OrderLifecycleState::PartialRemoteUnknown
    )));
}
