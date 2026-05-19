use super::*;
use crate::*;

#[cfg(test)]
async fn seed_test_plan(store: &InMemoryStore, execution_id: &str, account_id: &str) {
    store
        .save_plan_summary(&ExecutionPlanSummary {
            execution_id: execution_id.into(),
            account_id: pmx_core::AccountId(account_id.into()),
            normalized_intent_id: format!("norm-{execution_id}"),
            snapshot_id: format!("snap-{execution_id}"),
            decision_id: format!("decision-{execution_id}"),
            plan_hash: pmx_core::HashValue(format!("hash-{execution_id}")),
            status: pmx_core::PlanStatus::Ready,
            max_exposure: pmx_core::DecimalString("0".into()),
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
