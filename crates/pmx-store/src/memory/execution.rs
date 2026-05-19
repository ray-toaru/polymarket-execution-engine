use super::InMemoryStore;
use async_trait::async_trait;
use pmx_core::{
    ConstraintDecision, ExecutionPlanSummary, FeasibilitySnapshot, NormalizedIntent,
    OrderReservation, SubmitReceipt,
};

use crate::{ExecutionStore, StoreError};

#[path = "execution/decision.rs"]
mod decision;

#[path = "execution/plan.rs"]
mod plan;

#[path = "execution/reservation_receipt.rs"]
mod reservation_receipt;

#[path = "execution/snapshot.rs"]
mod snapshot;

#[async_trait]
impl ExecutionStore for InMemoryStore {
    async fn save_normalized_intent(&self, intent: &NormalizedIntent) -> Result<(), StoreError> {
        snapshot::save_normalized_intent(self, intent)
    }

    async fn load_normalized_intent(
        &self,
        normalized_intent_id: &str,
    ) -> Result<NormalizedIntent, StoreError> {
        snapshot::load_normalized_intent(self, normalized_intent_id)
    }

    async fn save_snapshot(&self, snapshot: &FeasibilitySnapshot) -> Result<(), StoreError> {
        snapshot::save_snapshot(self, snapshot)
    }

    async fn load_snapshot(&self, snapshot_id: &str) -> Result<FeasibilitySnapshot, StoreError> {
        snapshot::load_snapshot(self, snapshot_id)
    }

    async fn save_decision(&self, decision: &ConstraintDecision) -> Result<(), StoreError> {
        decision::save_decision(self, decision)
    }

    async fn load_decision(&self, decision_id: &str) -> Result<ConstraintDecision, StoreError> {
        decision::load_decision(self, decision_id)
    }

    async fn save_plan_summary(&self, plan: &ExecutionPlanSummary) -> Result<(), StoreError> {
        plan::save_plan_summary(self, plan)
    }

    async fn load_plan_summary(
        &self,
        execution_id: &str,
    ) -> Result<ExecutionPlanSummary, StoreError> {
        plan::load_plan_summary(self, execution_id)
    }

    async fn save_order_reservation(
        &self,
        reservation: &OrderReservation,
    ) -> Result<(), StoreError> {
        reservation_receipt::save_order_reservation(self, reservation)
    }

    async fn record_submit_receipt(&self, receipt: &SubmitReceipt) -> Result<(), StoreError> {
        reservation_receipt::record_submit_receipt(self, receipt)
    }

    async fn load_submit_receipt(&self, execution_id: &str) -> Result<SubmitReceipt, StoreError> {
        reservation_receipt::load_submit_receipt(self, execution_id)
    }
}
