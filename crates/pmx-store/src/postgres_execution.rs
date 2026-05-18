use async_trait::async_trait;
use pmx_core::{
    ConstraintDecision, ExecutionPlanSummary, FeasibilitySnapshot, NormalizedIntent,
    OrderReservation, SubmitReceipt,
};

mod object_graph;
mod receipt;
mod reservation;

use crate::postgres::PostgresStore;
use crate::{ExecutionStore, StoreError};

#[async_trait]
impl ExecutionStore for PostgresStore {
    async fn save_normalized_intent(&self, intent: &NormalizedIntent) -> Result<(), StoreError> {
        object_graph::save_normalized_intent(self, intent).await
    }

    async fn load_normalized_intent(
        &self,
        normalized_intent_id: &str,
    ) -> Result<NormalizedIntent, StoreError> {
        object_graph::load_normalized_intent(self, normalized_intent_id).await
    }

    async fn save_snapshot(&self, snapshot: &FeasibilitySnapshot) -> Result<(), StoreError> {
        object_graph::save_snapshot(self, snapshot).await
    }

    async fn load_snapshot(&self, snapshot_id: &str) -> Result<FeasibilitySnapshot, StoreError> {
        object_graph::load_snapshot(self, snapshot_id).await
    }

    async fn save_decision(&self, decision: &ConstraintDecision) -> Result<(), StoreError> {
        object_graph::save_decision(self, decision).await
    }

    async fn load_decision(&self, decision_id: &str) -> Result<ConstraintDecision, StoreError> {
        object_graph::load_decision(self, decision_id).await
    }

    async fn save_plan_summary(&self, plan: &ExecutionPlanSummary) -> Result<(), StoreError> {
        object_graph::save_plan_summary(self, plan).await
    }

    async fn load_plan_summary(
        &self,
        execution_id: &str,
    ) -> Result<ExecutionPlanSummary, StoreError> {
        object_graph::load_plan_summary(self, execution_id).await
    }

    async fn save_order_reservation(
        &self,
        reservation: &OrderReservation,
    ) -> Result<(), StoreError> {
        reservation::save_order_reservation(self, reservation).await
    }

    async fn record_submit_receipt(&self, receipt: &SubmitReceipt) -> Result<(), StoreError> {
        receipt::record_submit_receipt(self, receipt).await
    }

    async fn load_submit_receipt(&self, execution_id: &str) -> Result<SubmitReceipt, StoreError> {
        receipt::load_submit_receipt(self, execution_id).await
    }
}
