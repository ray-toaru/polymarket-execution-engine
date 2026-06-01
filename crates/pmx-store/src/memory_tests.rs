use super::*;
use crate::*;

fn hash_value(label: &str) -> pmx_core::HashValue {
    pmx_core::canonical_json_sha256(&format!("test-{label}")).expect("test hash")
}

#[cfg(test)]
async fn seed_test_plan(store: &InMemoryStore, execution_id: &str, account_id: &str) {
    store
        .save_plan_summary(&ExecutionPlanSummary {
            execution_id: execution_id.into(),
            account_id: pmx_core::AccountId(account_id.into()),
            normalized_intent_id: format!("norm-{execution_id}"),
            correlation_id: None,
            snapshot_id: format!("snap-{execution_id}"),
            snapshot_hash: hash_value(&format!("snap-{execution_id}")),
            decision_id: format!("decision-{execution_id}"),
            decision_hash: hash_value(&format!("decision-{execution_id}")),
            approval_id: format!("approval-{execution_id}"),
            approval_hash: hash_value(&format!("approval-{execution_id}")),
            plan_hash: hash_value(&format!("plan-{execution_id}")),
            status: pmx_core::PlanStatus::Ready,
            condition_id: pmx_core::ConditionId("cond-1".into()),
            token_id: pmx_core::TokenId("token-1".into()),
            side: pmx_core::Side::Buy,
            quantity_bound: pmx_core::QuantityBound::WorstCaseQuoteNotional(
                pmx_core::DecimalString("1".into()),
            ),
            limit_price: pmx_core::DecimalString("0.5".into()),
            time_in_force: pmx_core::TimeInForce::Gtc,
            collateral_profile_id: None,
            max_exposure: pmx_core::DecimalString("0".into()),
            executor_version: "test-executor".into(),
            contract_version: "1.0.0-draft".into(),
            explanation: vec!["test plan for sign-only lifecycle FK parity".into()],
        })
        .await
        .expect("seed execution plan");
}

#[path = "memory_tests/admin_sign_only.rs"]
mod admin_sign_only;
#[path = "memory_tests/common.rs"]
mod common;
#[path = "memory_tests/order_lifecycle.rs"]
mod order_lifecycle;
#[path = "memory_tests/real_funds_canary.rs"]
mod real_funds_canary;
#[path = "memory_tests/runtime_observation.rs"]
mod runtime_observation;
#[path = "memory_tests/runtime_worker_health.rs"]
mod runtime_worker_health;
