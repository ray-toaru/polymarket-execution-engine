use async_trait::async_trait;
use pmx_core::{
    ConstraintDecision, ExecutionPlanSummary, FeasibilitySnapshot, NormalizedIntent,
    OrderReservation, SubmitReceipt,
};

use crate::postgres::PostgresStore;
use crate::postgres_support::{load_json_payload, map_db_error};
use crate::{
    ExecutionStore, StoreError, advisory_lock_key, quantity_bound_to_resource_and_amount,
    reservation_state_to_str, submit_status_str,
};

#[async_trait]
impl ExecutionStore for PostgresStore {
    async fn save_normalized_intent(&self, intent: &NormalizedIntent) -> Result<(), StoreError> {
        let client = self.client().await?;
        let payload =
            serde_json::to_value(intent).map_err(|e| StoreError::InvalidData(e.to_string()))?;
        client
            .execute(
                "INSERT INTO normalized_intents (normalized_intent_id, intent_hash, account_id, payload) \
                 VALUES ($1, $2, $3, $4) \
                 ON CONFLICT (normalized_intent_id) DO UPDATE SET payload = EXCLUDED.payload",
                &[&intent.normalized_intent_id, &intent.intent_hash.0, &intent.account_id.0, &payload],
            )
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn load_normalized_intent(
        &self,
        normalized_intent_id: &str,
    ) -> Result<NormalizedIntent, StoreError> {
        let client = self.client().await?;
        load_json_payload(
            &client,
            "normalized_intents",
            "normalized_intent_id",
            normalized_intent_id,
            "payload",
        )
        .await
    }

    async fn save_snapshot(&self, snapshot: &FeasibilitySnapshot) -> Result<(), StoreError> {
        let client = self.client().await?;
        let payload =
            serde_json::to_value(snapshot).map_err(|e| StoreError::InvalidData(e.to_string()))?;
        client
            .execute(
                "INSERT INTO feasibility_snapshots (snapshot_id, snapshot_hash, normalized_intent_id, payload, captured_at) \
                 VALUES ($1, $2, $3, $4, $5) \
                 ON CONFLICT (snapshot_id) DO UPDATE SET payload = EXCLUDED.payload",
                &[
                    &snapshot.snapshot_id,
                    &snapshot.snapshot_hash.0,
                    &snapshot.normalized_intent_id,
                    &payload,
                    &snapshot.captured_at,
                ],
            )
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn load_snapshot(&self, snapshot_id: &str) -> Result<FeasibilitySnapshot, StoreError> {
        let client = self.client().await?;
        load_json_payload(
            &client,
            "feasibility_snapshots",
            "snapshot_id",
            snapshot_id,
            "payload",
        )
        .await
    }

    async fn save_decision(&self, decision: &ConstraintDecision) -> Result<(), StoreError> {
        let client = self.client().await?;
        let payload =
            serde_json::to_value(decision).map_err(|e| StoreError::InvalidData(e.to_string()))?;
        let reasons = serde_json::to_value(&decision.reasons)
            .map_err(|e| StoreError::InvalidData(e.to_string()))?;
        let snapshot_id: Option<String> = None;
        client
            .execute(
                "INSERT INTO constraint_decisions (decision_id, decision_hash, snapshot_id, status, reasons, payload) \
                 VALUES ($1, $2, $3, $4, $5, $6) \
                 ON CONFLICT (decision_id) DO UPDATE SET status = EXCLUDED.status, reasons = EXCLUDED.reasons, payload = EXCLUDED.payload",
                &[
                    &decision.decision_id,
                    &decision.decision_hash.0,
                    &snapshot_id,
                    &format!("{:?}", decision.status).to_uppercase(),
                    &reasons,
                    &payload,
                ],
            )
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn load_decision(&self, decision_id: &str) -> Result<ConstraintDecision, StoreError> {
        let client = self.client().await?;
        load_json_payload(
            &client,
            "constraint_decisions",
            "decision_id",
            decision_id,
            "payload",
        )
        .await
    }

    async fn save_plan_summary(&self, plan: &ExecutionPlanSummary) -> Result<(), StoreError> {
        let client = self.client().await?;
        let payload =
            serde_json::to_value(plan).map_err(|e| StoreError::InvalidData(e.to_string()))?;
        client
            .execute(
                "INSERT INTO execution_plans \
                 (execution_id, account_id, normalized_intent_id, snapshot_id, decision_id, plan_hash, status, summary_json) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
                 ON CONFLICT (execution_id) DO UPDATE SET \
                   account_id = EXCLUDED.account_id, \
                   normalized_intent_id = EXCLUDED.normalized_intent_id, \
                   snapshot_id = EXCLUDED.snapshot_id, \
                   decision_id = EXCLUDED.decision_id, \
                   plan_hash = EXCLUDED.plan_hash, \
                   status = EXCLUDED.status, \
                   summary_json = EXCLUDED.summary_json, \
                   updated_at = now()",
                &[
                    &plan.execution_id,
                    &plan.account_id.0,
                    &plan.normalized_intent_id,
                    &plan.snapshot_id,
                    &plan.decision_id,
                    &plan.plan_hash.0,
                    &format!("{:?}", plan.status).to_uppercase(),
                    &payload,
                ],
            )
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn load_plan_summary(
        &self,
        execution_id: &str,
    ) -> Result<ExecutionPlanSummary, StoreError> {
        let client = self.client().await?;
        load_json_payload(
            &client,
            "execution_plans",
            "execution_id",
            execution_id,
            "summary_json",
        )
        .await
    }

    async fn save_order_reservation(
        &self,
        reservation: &OrderReservation,
    ) -> Result<(), StoreError> {
        let (resource_kind, amount) =
            quantity_bound_to_resource_and_amount(&reservation.quantity_bound)?;
        let lock = advisory_lock_key(
            "reservation",
            &reservation.account_id.0,
            &format!("{}:{resource_kind}", reservation.execution_id.0),
        );
        let client = self.client().await?;
        client.batch_execute("BEGIN").await.map_err(map_db_error)?;
        if let Err(err) = client
            .execute("SELECT pg_advisory_xact_lock($1)", &[&lock.0])
            .await
        {
            Self::rollback(&client).await;
            return Err(map_db_error(err));
        }
        let order_id: Option<&str> = reservation.internal_order_id.as_ref().map(|v| v.0.as_str());
        let result = client
            .execute(
                "INSERT INTO order_reservations (reservation_id, order_id, execution_id, account_id, resource_kind, amount, state) \
                 VALUES ($1, $2, $3, $4, $5, $6::text::numeric, $7) \
                 ON CONFLICT (reservation_id) DO UPDATE SET state = EXCLUDED.state",
                &[
                    &reservation.reservation_id,
                    &order_id,
                    &reservation.execution_id.0,
                    &reservation.account_id.0,
                    &resource_kind,
                    &amount,
                    &reservation_state_to_str(&reservation.state),
                ],
            )
            .await;
        match result {
            Ok(_) => {
                client.batch_execute("COMMIT").await.map_err(map_db_error)?;
                Ok(())
            }
            Err(err) => {
                Self::rollback(&client).await;
                Err(map_db_error(err))
            }
        }
    }

    async fn record_submit_receipt(&self, receipt: &SubmitReceipt) -> Result<(), StoreError> {
        let client = self.client().await?;
        let payload =
            serde_json::to_value(receipt).map_err(|e| StoreError::InvalidData(e.to_string()))?;
        client
            .execute(
                "INSERT INTO submit_receipts (execution_id, receipt_id, status, executor_version, contract_version, response_json) \
                 VALUES ($1, $2, $3, $4, $5, $6) \
                 ON CONFLICT (execution_id) DO UPDATE SET receipt_id = EXCLUDED.receipt_id, status = EXCLUDED.status, response_json = EXCLUDED.response_json, updated_at = now()",
                &[
                    &receipt.execution_id,
                    &receipt.receipt_id,
                    &submit_status_str(&receipt.status),
                    &receipt.executor_version,
                    &receipt.contract_version,
                    &payload,
                ],
            )
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn load_submit_receipt(&self, execution_id: &str) -> Result<SubmitReceipt, StoreError> {
        let client = self.client().await?;
        load_json_payload(
            &client,
            "submit_receipts",
            "execution_id",
            execution_id,
            "response_json",
        )
        .await
    }
}
