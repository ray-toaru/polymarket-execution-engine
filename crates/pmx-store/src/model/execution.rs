use async_trait::async_trait;
use pmx_core::{
    ConstraintDecision, ExecutionPlanSummary, FeasibilitySnapshot, NormalizedIntent,
    OrderReservation, SubmitReceipt,
};

use super::StoreError;

#[async_trait]
pub trait ExecutionStore: Send + Sync {
    async fn save_normalized_intent(&self, intent: &NormalizedIntent) -> Result<(), StoreError>;
    async fn load_normalized_intent(
        &self,
        normalized_intent_id: &str,
    ) -> Result<NormalizedIntent, StoreError>;

    async fn save_snapshot(&self, snapshot: &FeasibilitySnapshot) -> Result<(), StoreError>;
    async fn load_snapshot(&self, snapshot_id: &str) -> Result<FeasibilitySnapshot, StoreError>;

    async fn save_decision(&self, decision: &ConstraintDecision) -> Result<(), StoreError>;
    async fn load_decision(&self, decision_id: &str) -> Result<ConstraintDecision, StoreError>;

    async fn save_plan_summary(&self, plan: &ExecutionPlanSummary) -> Result<(), StoreError>;
    async fn load_plan_summary(
        &self,
        execution_id: &str,
    ) -> Result<ExecutionPlanSummary, StoreError>;

    async fn save_order_reservation(
        &self,
        reservation: &OrderReservation,
    ) -> Result<(), StoreError>;
    async fn record_submit_receipt(&self, receipt: &SubmitReceipt) -> Result<(), StoreError>;
    async fn load_submit_receipt(&self, execution_id: &str) -> Result<SubmitReceipt, StoreError>;
}
