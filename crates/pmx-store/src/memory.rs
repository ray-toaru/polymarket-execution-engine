use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pmx_core::{
    CollateralProfileStatus, ConstraintDecision, ExecutionPlanSummary, FeasibilitySnapshot,
    GeoblockStatus, NormalizedIntent, OrderReservation, RuntimeStateSummary,
    SignOnlyLifecycleRecord, SubmitReceipt, WorkerStatus, lifecycle_requires_reconcile,
    transition_order_state,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::{
    AdminAuditEvent, AdminAuditQuery, AdminAuditStore, ExecutionLifecycleEvent,
    ExecutionLifecycleQuery, ExecutionLifecycleStore, ExecutionStore, IdempotencyAction,
    IdempotencyStore, OrderLifecycleEventQuery, OrderLifecycleEventRecord, OrderLifecycleRecord,
    OrderLifecycleStore, OrderReconcileBacklogQuery, OrderReconcileBacklogStore, RuntimeStateQuery,
    RuntimeStateStore, RuntimeWorkerHealthStore, RuntimeWorkerHeartbeat, RuntimeWorkerObservation,
    RuntimeWorkerObservationStore, RuntimeWorkerStatusQuery, RuntimeWorkerStatusReport,
    RuntimeWorkerStatusStore, SignOnlyLifecycleQuery, SignOnlyLifecycleStore, StoreError,
    apply_runtime_worker_observations, runtime_observation_is_fresh, sanitize_admin_audit_event,
    sanitize_execution_lifecycle_event, sanitize_sign_only_lifecycle_record,
    sign_only_lifecycle_record_is_replay, validate_sign_only_lifecycle_append_for_store,
    worker_status_from_heartbeats,
};

#[derive(Clone, Default)]
pub struct InMemoryStore {
    inner: Arc<Mutex<InMemoryState>>,
}

#[derive(Default)]
struct InMemoryState {
    normalized: HashMap<String, NormalizedIntent>,
    snapshots: HashMap<String, FeasibilitySnapshot>,
    decisions: HashMap<String, ConstraintDecision>,
    plans: HashMap<String, ExecutionPlanSummary>,
    reservations: HashMap<String, OrderReservation>,
    receipts: HashMap<String, SubmitReceipt>,
    idempotency: HashMap<String, IdempotencyRecord>,
    attempt_counters: HashMap<String, u32>,
    admin_audit: Vec<AdminAuditEvent>,
    admin_audit_counter: i64,
    runtime_states: HashMap<String, RuntimeStateSummary>,
    lifecycle_events: Vec<ExecutionLifecycleEvent>,
    lifecycle_event_counter: i64,
    sign_only_lifecycle_events: Vec<SignOnlyLifecycleRecord>,
    sign_only_event_counter: i64,
    runtime_worker_observations: Vec<RuntimeWorkerObservation>,
    worker_health: HashMap<String, RuntimeWorkerHeartbeat>,
    orders: HashMap<String, OrderLifecycleRecord>,
    order_events: Vec<OrderLifecycleEventRecord>,
    order_event_counter: i64,
}

#[derive(Clone)]
struct IdempotencyRecord {
    submit_attempt: u32,
    request_fingerprint: String,
    response_fingerprint: Option<String>,
    response_json: Option<String>,
}

impl InMemoryStore {
    pub fn set_runtime_state_for_test(
        &self,
        account_id: &str,
        condition_id: &str,
        collateral_profile_id: Option<&str>,
        runtime_state: RuntimeStateSummary,
    ) {
        let query = RuntimeStateQuery {
            account_id: account_id.to_owned(),
            condition_id: condition_id.to_owned(),
            collateral_profile_id: collateral_profile_id.map(ToOwned::to_owned),
            required_capabilities: vec![],
        };
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .runtime_states
            .insert(query.key(), runtime_state);
    }

    fn observations_for_account(&self, account_id: &str) -> Vec<RuntimeWorkerObservation> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .runtime_worker_observations
            .iter()
            .filter(|observation| {
                observation.account_id == account_id && runtime_observation_is_fresh(observation)
            })
            .cloned()
            .collect()
    }
}

fn identity(account_id: &str, execution_id: &str, idempotency_key: &str) -> String {
    format!("{account_id}\u{1f}{execution_id}\u{1f}{idempotency_key}")
}

fn attempt_counter_key(account_id: &str, execution_id: &str) -> String {
    format!("{account_id}\u{1f}{execution_id}")
}

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

#[async_trait]
impl ExecutionLifecycleStore for InMemoryStore {
    async fn record_execution_lifecycle_event(
        &self,
        event: &ExecutionLifecycleEvent,
    ) -> Result<(), StoreError> {
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        state.lifecycle_event_counter += 1;
        let mut stored = sanitize_execution_lifecycle_event(event.clone());
        stored.event_id = Some(state.lifecycle_event_counter);
        stored.created_at = Some(Utc::now());
        state.lifecycle_events.push(stored);
        Ok(())
    }

    async fn list_execution_lifecycle_events(
        &self,
        query: &ExecutionLifecycleQuery,
    ) -> Result<Vec<ExecutionLifecycleEvent>, StoreError> {
        let mut events: Vec<_> = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .lifecycle_events
            .iter()
            .filter(|event| event.execution_id == query.execution_id)
            .filter(|event| {
                query
                    .before_event_id
                    .map(|before| event.event_id.unwrap_or(i64::MAX) < before)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        events.sort_by_key(|event| event.event_id.unwrap_or(0));
        events.reverse();
        events.truncate(query.bounded_limit());
        events.reverse();
        Ok(events)
    }
}

#[async_trait]
impl SignOnlyLifecycleStore for InMemoryStore {
    async fn record_sign_only_lifecycle_event(
        &self,
        record: &SignOnlyLifecycleRecord,
    ) -> Result<(), StoreError> {
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        if !state.plans.contains_key(&record.execution_id.0) {
            return Err(StoreError::NotFound(format!(
                "execution_id={}",
                record.execution_id.0
            )));
        }
        let existing: Vec<_> = state
            .sign_only_lifecycle_events
            .iter()
            .filter(|existing| existing.execution_id == record.execution_id)
            .cloned()
            .collect();
        if sign_only_lifecycle_record_is_replay(&existing, record)? {
            return Ok(());
        }
        validate_sign_only_lifecycle_append_for_store(&existing, record)?;
        state.sign_only_event_counter += 1;
        let mut stored = sanitize_sign_only_lifecycle_record(record.clone());
        stored.event_id = Some(state.sign_only_event_counter);
        stored.created_at = Some(Utc::now());
        state.sign_only_lifecycle_events.push(stored);
        Ok(())
    }

    async fn list_sign_only_lifecycle_events(
        &self,
        query: &SignOnlyLifecycleQuery,
    ) -> Result<Vec<SignOnlyLifecycleRecord>, StoreError> {
        let mut records: Vec<_> = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .sign_only_lifecycle_events
            .iter()
            .filter(|record| record.execution_id.0 == query.execution_id)
            .filter(|record| {
                query
                    .before_event_id
                    .map(|before| record.event_id.unwrap_or(i64::MAX) < before)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        records.sort_by_key(|record| record.event_id.unwrap_or(0));
        records.reverse();
        records.truncate(query.bounded_limit());
        records.reverse();
        Ok(records)
    }
}

#[async_trait]
impl RuntimeWorkerObservationStore for InMemoryStore {
    async fn record_runtime_worker_observation(
        &self,
        observation: &RuntimeWorkerObservation,
    ) -> Result<(), StoreError> {
        let mut stored = observation.clone();
        if stored.observed_at.is_none() {
            stored.observed_at = Some(Utc::now());
        }
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .runtime_worker_observations
            .push(stored);
        Ok(())
    }
}

#[async_trait]
impl RuntimeWorkerHealthStore for InMemoryStore {
    async fn record_worker_heartbeat(
        &self,
        heartbeat: &RuntimeWorkerHeartbeat,
    ) -> Result<(), StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .worker_health
            .insert(heartbeat.worker_id.clone(), heartbeat.clone());
        Ok(())
    }
}

#[async_trait]
impl RuntimeWorkerStatusStore for InMemoryStore {
    async fn list_runtime_worker_status(
        &self,
        query: &RuntimeWorkerStatusQuery,
    ) -> Result<RuntimeWorkerStatusReport, StoreError> {
        let state = self.inner.lock().expect("in-memory store mutex poisoned");
        let mut heartbeats: Vec<_> = state.worker_health.values().cloned().collect();
        heartbeats.sort_by(|left, right| {
            right
                .last_heartbeat_at
                .cmp(&left.last_heartbeat_at)
                .then_with(|| left.worker_id.cmp(&right.worker_id))
        });
        heartbeats.truncate(query.bounded_limit());

        let mut observations: Vec<_> = state
            .runtime_worker_observations
            .iter()
            .filter(|observation| observation.account_id == query.account_id)
            .filter(|observation| {
                query
                    .before_observed_at
                    .map(|before| {
                        observation.observed_at.unwrap_or(DateTime::<Utc>::MAX_UTC) < before
                    })
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        observations.sort_by(|left, right| {
            right
                .observed_at
                .cmp(&left.observed_at)
                .then_with(|| left.capability.cmp(&right.capability))
        });
        observations.truncate(query.bounded_limit());
        observations.reverse();
        Ok(RuntimeWorkerStatusReport {
            heartbeats,
            observations,
        })
    }
}

#[async_trait]
impl RuntimeStateStore for InMemoryStore {
    async fn load_runtime_state(
        &self,
        query: &RuntimeStateQuery,
    ) -> Result<RuntimeStateSummary, StoreError> {
        let mut base = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .runtime_states
            .get(&query.key())
            .cloned()
            .unwrap_or(RuntimeStateSummary {
                geoblock_status: GeoblockStatus::Unknown,
                worker_status: WorkerStatus::Unknown,
                collateral_profile_status: CollateralProfileStatus::Unknown,
                kill_switch_enabled: true,
                required_capabilities: query.required_capabilities.clone(),
            });
        let mut required_capabilities = query.required_capabilities.clone();
        if required_capabilities.is_empty() {
            required_capabilities = base.required_capabilities.clone();
        }
        let heartbeats: Vec<_> = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .worker_health
            .values()
            .cloned()
            .collect();
        if !required_capabilities.is_empty() {
            base.worker_status = worker_status_from_heartbeats(&heartbeats, &required_capabilities);
            base.required_capabilities = required_capabilities;
        }
        Ok(apply_runtime_worker_observations(
            base,
            &self.observations_for_account(&query.account_id),
        ))
    }
}

#[async_trait]
impl OrderLifecycleStore for InMemoryStore {
    async fn upsert_order_lifecycle(&self, order: &OrderLifecycleRecord) -> Result<(), StoreError> {
        let mut stored = order.clone();
        let now = Utc::now();
        if stored.created_at.is_none() {
            stored.created_at = Some(now);
        }
        stored.updated_at = Some(now);
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .orders
            .insert(stored.order_id.clone(), stored);
        Ok(())
    }

    async fn record_order_lifecycle_event(
        &self,
        event: &OrderLifecycleEventRecord,
    ) -> Result<OrderLifecycleRecord, StoreError> {
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        let Some(order) = state.orders.get_mut(&event.order_id) else {
            return Err(StoreError::NotFound(format!("order_id={}", event.order_id)));
        };
        let next = transition_order_state(order.lifecycle_state.clone(), event.event.clone())
            .map_err(|err| StoreError::Conflict(err.to_string()))?;
        order.lifecycle_state = next;
        order.updated_at = Some(Utc::now());
        let updated = order.clone();
        state.order_event_counter += 1;
        let mut stored_event = event.clone();
        stored_event.event_id = Some(state.order_event_counter);
        stored_event.created_at = Some(Utc::now());
        state.order_events.push(stored_event);
        Ok(updated)
    }

    async fn load_order_lifecycle(
        &self,
        order_id: &str,
    ) -> Result<Option<OrderLifecycleRecord>, StoreError> {
        Ok(self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .orders
            .get(order_id)
            .cloned())
    }

    async fn list_order_lifecycle_events(
        &self,
        query: &OrderLifecycleEventQuery,
    ) -> Result<Vec<OrderLifecycleEventRecord>, StoreError> {
        let mut events: Vec<_> = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .order_events
            .iter()
            .filter(|event| event.order_id == query.order_id)
            .filter(|event| {
                query
                    .before_event_id
                    .map(|before| event.event_id.unwrap_or(i64::MAX) < before)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        events.sort_by_key(|event| event.event_id.unwrap_or(0));
        events.reverse();
        events.truncate(query.bounded_limit());
        events.reverse();
        Ok(events)
    }
}

#[async_trait]
impl OrderReconcileBacklogStore for InMemoryStore {
    async fn list_reconcile_backlog_orders(
        &self,
        query: &OrderReconcileBacklogQuery,
    ) -> Result<Vec<OrderLifecycleRecord>, StoreError> {
        let mut orders: Vec<_> = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .orders
            .values()
            .filter(|order| order.account_id == query.account_id)
            .filter(|order| lifecycle_requires_reconcile(&order.lifecycle_state))
            .cloned()
            .collect();
        orders.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.order_id.cmp(&right.order_id))
        });
        orders.truncate(query.bounded_limit());
        Ok(orders)
    }
}

#[async_trait]
impl AdminAuditStore for InMemoryStore {
    async fn record_admin_audit_event(&self, event: &AdminAuditEvent) -> Result<(), StoreError> {
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        state.admin_audit_counter += 1;
        let mut stored = sanitize_admin_audit_event(event.clone());
        stored.audit_id = Some(state.admin_audit_counter);
        stored.created_at = Some(Utc::now());
        state.admin_audit.push(stored);
        Ok(())
    }

    async fn list_admin_audit_events(
        &self,
        query: &AdminAuditQuery,
    ) -> Result<Vec<AdminAuditEvent>, StoreError> {
        let mut events: Vec<_> = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .admin_audit
            .iter()
            .filter(|event| {
                query
                    .before_audit_id
                    .map(|before| event.audit_id.unwrap_or(i64::MAX) < before)
                    .unwrap_or(true)
            })
            .filter(|event| {
                query
                    .operation
                    .as_ref()
                    .map(|operation| &event.operation == operation)
                    .unwrap_or(true)
            })
            .filter(|event| {
                query
                    .principal_subject
                    .as_ref()
                    .map(|principal_subject| &event.principal_subject == principal_subject)
                    .unwrap_or(true)
            })
            .filter(|event| {
                query
                    .result
                    .as_ref()
                    .map(|result| &event.result == result)
                    .unwrap_or(true)
            })
            .filter(|event| {
                query
                    .correlation_id
                    .as_ref()
                    .map(|correlation_id| event.correlation_id.as_ref() == Some(correlation_id))
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        events.sort_by_key(|event| event.audit_id.unwrap_or(0));
        events.reverse();
        events.truncate(query.bounded_limit());
        events.reverse();
        Ok(events)
    }
}

#[async_trait]
impl IdempotencyStore for InMemoryStore {
    async fn begin_submit_attempt(
        &self,
        account_id: &str,
        execution_id: &str,
        idempotency_key: &str,
        request_fingerprint: &str,
    ) -> Result<IdempotencyAction, StoreError> {
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        let key = identity(account_id, execution_id, idempotency_key);
        if let Some(existing) = state.idempotency.get(&key) {
            if existing.request_fingerprint != request_fingerprint {
                return Ok(IdempotencyAction::Conflict);
            }
            if let (Some(response_fingerprint), Some(response_json)) =
                (&existing.response_fingerprint, &existing.response_json)
            {
                return Ok(IdempotencyAction::ReplayStoredResponse {
                    response_fingerprint: response_fingerprint.clone(),
                    response_json: response_json.clone(),
                });
            }
            return Ok(IdempotencyAction::InProgress {
                submit_attempt: existing.submit_attempt,
                retry_after_ms: 1_000,
            });
        }

        let counter_key = attempt_counter_key(account_id, execution_id);
        let next_attempt = state
            .attempt_counters
            .get(&counter_key)
            .copied()
            .unwrap_or(0)
            + 1;
        state.attempt_counters.insert(counter_key, next_attempt);
        state.idempotency.insert(
            key,
            IdempotencyRecord {
                submit_attempt: next_attempt,
                request_fingerprint: request_fingerprint.into(),
                response_fingerprint: None,
                response_json: None,
            },
        );
        Ok(IdempotencyAction::Proceed {
            submit_attempt: next_attempt,
            owner_token: format!("owner-{account_id}-{execution_id}-{next_attempt}"),
        })
    }

    async fn finish_submit_attempt(
        &self,
        account_id: &str,
        execution_id: &str,
        idempotency_key: &str,
        request_fingerprint: &str,
        response_fingerprint: &str,
        response_json: &str,
    ) -> Result<(), StoreError> {
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        let key = identity(account_id, execution_id, idempotency_key);
        let record = state
            .idempotency
            .get_mut(&key)
            .ok_or_else(|| StoreError::NotFound(key.clone()))?;
        if record.request_fingerprint != request_fingerprint {
            return Err(StoreError::Conflict("request_fingerprint mismatch".into()));
        }
        record.response_fingerprint = Some(response_fingerprint.into());
        record.response_json = Some(response_json.into());
        Ok(())
    }
}

#[cfg(test)]
#[path = "memory_tests.rs"]
mod memory_tests;
