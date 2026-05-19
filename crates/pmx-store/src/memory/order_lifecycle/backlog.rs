use super::*;

pub fn list_reconcile_backlog_orders(
    store: &InMemoryStore,
    query: &OrderReconcileBacklogQuery,
) -> Result<Vec<OrderLifecycleRecord>, StoreError> {
    let mut orders: Vec<_> = store
        .inner
        .lock()
        .expect("in-memory store mutex poisoned")
        .orders
        .values()
        .filter(|order| order.account_id == query.account_id)
        .filter(|order| lifecycle_requires_reconcile(&order.lifecycle_state))
        .cloned()
        .collect();
    orders.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.order_id.cmp(&right.order_id))
    });
    orders.truncate(query.bounded_limit());
    Ok(orders)
}
