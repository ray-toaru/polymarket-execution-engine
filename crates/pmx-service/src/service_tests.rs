use super::*;
use crate::*;
use crate::{StaticRuntimeStateProvider, StoreBackedRuntimeStateProvider};
use chrono::{Duration, Utc};
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

fn hash_value(label: &str) -> HashValue {
    canonical_json_sha256(&format!("test-{label}")).expect("test hash")
}

fn approval_for(snapshot: &FeasibilitySnapshot, decision: &ConstraintDecision) -> ApprovalReceipt {
    let mut approval = ApprovalReceipt {
        approval_id: format!("approval-{}", snapshot.snapshot_id),
        approved_by: "operator".into(),
        approved_at: Utc::now() - Duration::seconds(1),
        expires_at: Utc::now() + Duration::hours(1),
        approval_scope: ApprovalScope::Shadow,
        approval_hash: zero_hash(),
        bound_artifact_sha256: hash_value("artifact"),
        bound_evidence_manifest_sha256: hash_value("evidence-manifest"),
        bound_snapshot_hash: snapshot.snapshot_hash.clone(),
        bound_decision_hash: decision.decision_hash.clone(),
        bound_plan_hash: None,
        operator_identity_ref: "local-test-operator".into(),
    };
    approval.approval_hash = approval_receipt_hash(&approval).expect("approval hash");
    approval
}

fn zero_hash() -> HashValue {
    HashValue::from_sha256_hex("0000000000000000000000000000000000000000000000000000000000000000")
        .expect("valid zero hash")
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

async fn seed_test_plan(store: &InMemoryStore, execution_id: &str, account_id: &str) -> String {
    let plan_hash = hash_value(&format!("plan-{execution_id}"));
    store
        .save_plan_summary(&ExecutionPlanSummary {
            execution_id: execution_id.into(),
            account_id: AccountId(account_id.into()),
            normalized_intent_id: format!("norm-{execution_id}"),
            correlation_id: None,
            snapshot_id: format!("snap-{execution_id}"),
            snapshot_hash: hash_value(&format!("snap-{execution_id}")),
            decision_id: format!("decision-{execution_id}"),
            decision_hash: hash_value(&format!("decision-{execution_id}")),
            approval_id: format!("approval-{execution_id}"),
            approval_hash: hash_value(&format!("approval-{execution_id}")),
            plan_hash: plan_hash.clone(),
            status: PlanStatus::Ready,
            condition_id: ConditionId("cond-1".into()),
            token_id: TokenId("token-1".into()),
            side: Side::Buy,
            quantity_bound: QuantityBound::WorstCaseQuoteNotional(DecimalString("1".into())),
            limit_price: DecimalString("0.5".into()),
            time_in_force: TimeInForce::Gtc,
            collateral_profile_id: None,
            max_exposure: DecimalString("0".into()),
            executor_version: "test-executor".into(),
            contract_version: DEFAULT_CONTRACT_VERSION.into(),
            explanation: vec!["test plan for sign-only lifecycle FK parity".into()],
        })
        .await
        .expect("seed execution plan");
    plan_hash.0
}

#[path = "service_tests/flow.rs"]
mod flow;

#[path = "service_tests/non_live_order_lifecycle.rs"]
mod non_live_order_lifecycle;

#[path = "service_tests/real_funds_canary.rs"]
mod real_funds_canary;

#[path = "service_tests/runtime_worker_basic.rs"]
mod runtime_worker_basic;

#[path = "service_tests/runtime_worker_lease.rs"]
mod runtime_worker_lease;

#[path = "service_tests/runtime_worker_specialized.rs"]
mod runtime_worker_specialized;

#[path = "service_tests/sign_only.rs"]
mod sign_only;
