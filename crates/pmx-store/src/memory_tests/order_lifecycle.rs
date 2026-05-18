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

#[path = "order_lifecycle/backlog.rs"]
mod backlog;

#[path = "order_lifecycle/cancel_requested.rs"]
mod cancel_requested;

#[path = "order_lifecycle/rejects.rs"]
mod rejects;

#[path = "order_lifecycle/replay.rs"]
mod replay;
