use super::*;
use crate::*;
use crate::{StaticRuntimeStateProvider, StoreBackedRuntimeStateProvider};
use chrono::Utc;
use pmx_policy::evaluate_constraints;
use pmx_runtime::{HeartbeatLeaseCandidate, RuntimeSignal};
use pmx_store::{
    InMemoryStore, OrderLifecycleStore, PostgresStore, RuntimeStateQuery, RuntimeStateStore,
    RuntimeWorkerHealthStore, RuntimeWorkerHeartbeat, RuntimeWorkerStatusQuery,
    RuntimeWorkerStatusStore,
};

fn intent() -> TradeIntent {
    TradeIntent {
        client_intent_id: "client-1".into(),
        account_id: AccountId("acct-1".into()),
        market: MarketRef {
            condition_id: ConditionId("cond-1".into()),
            slug: Some("slug".into()),
            is_sports: false,
        },
        token_id: TokenId("token-1".into()),
        side: Side::Buy,
        quantity: QuantityIntent {
            max_notional: Some(DecimalString("1".into())),
            max_shares: None,
        },
        limit_price: DecimalString("0.5".into()),
        time_in_force: TimeInForce::Gtc,
        collateral_profile_id: None,
    }
}

fn allow_runtime_state() -> RuntimeStateSummary {
    RuntimeStateSummary {
        geoblock_status: GeoblockStatus::Allowed,
        worker_status: WorkerStatus::Healthy,
        collateral_profile_status: CollateralProfileStatus::DefaultResolved,
        kill_switch_enabled: false,
        required_capabilities: vec![],
    }
}

fn approval() -> ApprovalReceipt {
    ApprovalReceipt {
        approval_id: "approval-1".into(),
        approved_by: "operator".into(),
        approved_at: Utc::now(),
        approval_hash: HashValue("approval-hash".into()),
    }
}

fn order(order_id: &str, lifecycle_state: OrderLifecycleState) -> OrderLifecycleRecord {
    OrderLifecycleRecord {
        order_id: order_id.into(),
        execution_id: "exec-order-life".into(),
        account_id: "acct-1".into(),
        condition_id: "cond-1".into(),
        token_id: "token-1".into(),
        side: "BUY".into(),
        lifecycle_state,
        remote_order_id: Some(format!("remote-{order_id}")),
        remote_state: Some("OPEN".into()),
        created_at: None,
        updated_at: None,
    }
}

async fn seed_test_plan(store: &InMemoryStore, execution_id: &str, account_id: &str) {
    store
        .save_plan_summary(&ExecutionPlanSummary {
            execution_id: execution_id.into(),
            account_id: AccountId(account_id.into()),
            normalized_intent_id: format!("norm-{execution_id}"),
            snapshot_id: format!("snap-{execution_id}"),
            decision_id: format!("decision-{execution_id}"),
            plan_hash: HashValue(format!("hash-{execution_id}")),
            status: PlanStatus::Ready,
            max_exposure: DecimalString("0".into()),
            explanation: vec!["test plan for sign-only lifecycle FK parity".into()],
        })
        .await
        .expect("seed execution plan");
}

#[path = "service_tests/flow.rs"]
mod flow;

#[path = "service_tests/non_live_order_lifecycle.rs"]
mod non_live_order_lifecycle;

#[path = "service_tests/runtime_worker_basic.rs"]
mod runtime_worker_basic;

#[path = "service_tests/runtime_worker_lease.rs"]
mod runtime_worker_lease;

#[path = "service_tests/runtime_worker_specialized.rs"]
mod runtime_worker_specialized;

#[path = "service_tests/sign_only.rs"]
mod sign_only;
