use super::InMemoryStore;
use async_trait::async_trait;
use pmx_core::{
    ConstraintDecision, ExecutionPlanSummary, FeasibilitySnapshot, NormalizedIntent,
    OrderReservation, SubmitReceipt,
};

use crate::{ExecutionStore, StoreError};

#[async_trait]
impl ExecutionStore for InMemoryStore {
    async fn save_normalized_intent(&self, intent: &NormalizedIntent) -> Result<(), StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .normalized
            .insert(intent.normalized_intent_id.clone(), intent.clone());
        Ok(())
    }

    async fn load_normalized_intent(
        &self,
        normalized_intent_id: &str,
    ) -> Result<NormalizedIntent, StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .normalized
            .get(normalized_intent_id)
            .cloned()
            .ok_or_else(|| {
                StoreError::NotFound(format!("normalized_intent_id={normalized_intent_id}"))
            })
    }

    async fn save_snapshot(&self, snapshot: &FeasibilitySnapshot) -> Result<(), StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .snapshots
            .insert(snapshot.snapshot_id.clone(), snapshot.clone());
        Ok(())
    }

    async fn load_snapshot(&self, snapshot_id: &str) -> Result<FeasibilitySnapshot, StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .snapshots
            .get(snapshot_id)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(format!("snapshot_id={snapshot_id}")))
    }

    async fn save_decision(&self, decision: &ConstraintDecision) -> Result<(), StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .decisions
            .insert(decision.decision_id.clone(), decision.clone());
        Ok(())
    }

    async fn load_decision(&self, decision_id: &str) -> Result<ConstraintDecision, StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .decisions
            .get(decision_id)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(format!("decision_id={decision_id}")))
    }

    async fn save_plan_summary(&self, plan: &ExecutionPlanSummary) -> Result<(), StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .plans
            .insert(plan.execution_id.clone(), plan.clone());
        Ok(())
    }

    async fn load_plan_summary(
        &self,
        execution_id: &str,
    ) -> Result<ExecutionPlanSummary, StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .plans
            .get(execution_id)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(format!("execution_id={execution_id}")))
    }

    async fn save_order_reservation(
        &self,
        reservation: &OrderReservation,
    ) -> Result<(), StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .reservations
            .insert(reservation.reservation_id.clone(), reservation.clone());
        Ok(())
    }

    async fn record_submit_receipt(&self, receipt: &SubmitReceipt) -> Result<(), StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .receipts
            .insert(receipt.execution_id.clone(), receipt.clone());
        Ok(())
    }

    async fn load_submit_receipt(&self, execution_id: &str) -> Result<SubmitReceipt, StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .receipts
            .get(execution_id)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(format!("execution_id={execution_id}")))
    }
}
