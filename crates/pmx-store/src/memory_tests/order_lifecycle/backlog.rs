use super::*;

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
