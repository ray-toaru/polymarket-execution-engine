#[cfg(test)]
use chrono::Utc;
#[cfg(test)]
use pmx_core::{CollateralProfileStatus, GeoblockStatus, WorkerStatus};
use pmx_core::{
    ConstraintDecision, ExecutionPlanSummary, FeasibilitySnapshot, NormalizedIntent,
    OrderReservation, RuntimeStateSummary, SignOnlyLifecycleRecord, SubmitReceipt,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

mod audit;
mod execution;
mod idempotency;
mod lifecycle;
mod order_lifecycle;
mod runtime;

use crate::{
    AdminAuditEvent, ExecutionLifecycleEvent, OrderLifecycleEventRecord, OrderLifecycleRecord,
    RuntimeWorkerHeartbeat, RuntimeWorkerObservation,
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

fn identity(account_id: &str, execution_id: &str, idempotency_key: &str) -> String {
    format!("{account_id}\u{1f}{execution_id}\u{1f}{idempotency_key}")
}

fn attempt_counter_key(account_id: &str, execution_id: &str) -> String {
    format!("{account_id}\u{1f}{execution_id}")
}

#[cfg(test)]
#[path = "memory_tests.rs"]
mod memory_tests;
