use chrono::{Duration, Utc};
use pmx_core::{
    OrderEventKind, OrderLifecycleState, QuantityBound, ReservationState, RuntimeStateSummary,
    SignOnlyLifecycleRecord, SignOnlyLifecycleState, SubmitStatus, WorkerStatus,
    sign_only_lifecycle_records_equivalent, transition_sign_only_lifecycle,
};

use crate::{
    AdminAuditEvent, ExecutionLifecycleEvent, RuntimeWorkerHeartbeat, RuntimeWorkerObservation,
    StoreError,
};

pub const DEFAULT_RUNTIME_OBSERVATION_TTL_SECONDS: i64 = 120;
pub const RUNTIME_OBSERVATION_TTL_SECONDS: i64 = DEFAULT_RUNTIME_OBSERVATION_TTL_SECONDS;

/// Runtime observation freshness horizon.
///
/// The default is intentionally conservative and remains configurable because v0.23 has not yet
/// established a validated worker heartbeat cadence. Invalid or non-positive values fail closed
/// back to the default instead of silently extending freshness.
pub fn runtime_observation_ttl_seconds() -> i64 {
    std::env::var("PMX_RUNTIME_OBSERVATION_TTL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0 && *value <= 86_400)
        .unwrap_or(DEFAULT_RUNTIME_OBSERVATION_TTL_SECONDS)
}

pub(crate) fn runtime_observation_is_fresh(observation: &RuntimeWorkerObservation) -> bool {
    observation
        .observed_at
        .map(|observed_at| {
            observed_at >= Utc::now() - Duration::seconds(runtime_observation_ttl_seconds())
        })
        .unwrap_or(true)
}

fn runtime_worker_heartbeat_is_fresh(heartbeat: &RuntimeWorkerHeartbeat) -> bool {
    heartbeat.last_heartbeat_at >= Utc::now() - Duration::seconds(runtime_observation_ttl_seconds())
}

pub(crate) fn worker_status_from_heartbeats(
    heartbeats: &[RuntimeWorkerHeartbeat],
    required_capabilities: &[String],
) -> WorkerStatus {
    if required_capabilities.is_empty() {
        return WorkerStatus::Healthy;
    }
    let mut degraded = false;
    for capability in required_capabilities {
        let Some(heartbeat) = heartbeats
            .iter()
            .filter(|heartbeat| &heartbeat.capability == capability)
            .max_by_key(|heartbeat| heartbeat.last_heartbeat_at)
        else {
            return WorkerStatus::Unknown;
        };
        let normalized = heartbeat.status.trim().to_ascii_uppercase();
        if !runtime_worker_heartbeat_is_fresh(heartbeat)
            || matches!(normalized.as_str(), "STALE" | "ERROR" | "DOWN")
        {
            return WorkerStatus::Stale;
        }
        if normalized == "DEGRADED" {
            degraded = true;
        } else if normalized != "HEALTHY" {
            return WorkerStatus::Unknown;
        }
    }
    if degraded {
        WorkerStatus::Degraded
    } else {
        WorkerStatus::Healthy
    }
}

pub(crate) fn sanitize_admin_audit_event(mut event: AdminAuditEvent) -> AdminAuditEvent {
    event.audit_id = None;
    event.created_at = None;
    event
}

pub(crate) fn sanitize_execution_lifecycle_event(
    mut event: ExecutionLifecycleEvent,
) -> ExecutionLifecycleEvent {
    event.event_id = None;
    event.created_at = None;
    event
}

pub(crate) fn sanitize_sign_only_lifecycle_record(
    mut record: SignOnlyLifecycleRecord,
) -> SignOnlyLifecycleRecord {
    record.event_id = None;
    record.created_at = None;
    record
}

pub(crate) fn sign_only_lifecycle_record_is_replay(
    existing: &[SignOnlyLifecycleRecord],
    record: &SignOnlyLifecycleRecord,
) -> Result<bool, StoreError> {
    if let Some(client_event_id) = record.client_event_id.as_deref() {
        if client_event_id.trim().is_empty() {
            return Err(StoreError::Conflict(
                "sign-only lifecycle client_event_id must not be empty".into(),
            ));
        }
        if let Some(previous) = existing
            .iter()
            .find(|candidate| candidate.client_event_id.as_deref() == Some(client_event_id))
        {
            if sign_only_lifecycle_records_equivalent(previous, record) {
                return Ok(true);
            }
            return Err(StoreError::Conflict(
                "sign-only lifecycle client_event_id reused with different event payload".into(),
            ));
        }
    }
    Ok(existing
        .last()
        .map(|last| sign_only_lifecycle_records_equivalent(last, record))
        .unwrap_or(false))
}

pub(crate) fn validate_sign_only_lifecycle_append_for_store(
    existing: &[SignOnlyLifecycleRecord],
    record: &SignOnlyLifecycleRecord,
) -> Result<(), StoreError> {
    if !record.no_remote_side_effect {
        return Err(StoreError::Conflict(
            "sign-only lifecycle record must not contain remote side effects".into(),
        ));
    }
    if sign_only_lifecycle_record_is_replay(existing, record)? {
        return Ok(());
    }
    if let Some(first) = existing.first()
        && first.account_id != record.account_id
    {
        return Err(StoreError::Conflict(
            "sign-only lifecycle account_id does not match existing execution history".into(),
        ));
    }
    let from = existing
        .last()
        .map(|event| event.state.clone())
        .unwrap_or(SignOnlyLifecycleState::Planned);
    if matches!(
        from,
        SignOnlyLifecycleState::SignedDryRun
            | SignOnlyLifecycleState::Failed
            | SignOnlyLifecycleState::Abandoned
    ) {
        return Err(StoreError::Conflict(
            "sign-only lifecycle is already terminal".into(),
        ));
    }
    let expected = transition_sign_only_lifecycle(from.clone(), record.event.clone())
        .map_err(|err| StoreError::Conflict(err.to_string()))?;
    if expected != record.state {
        return Err(StoreError::Conflict(format!(
            "sign-only lifecycle state mismatch: event {:?} from {:?} yields {:?}, got {:?}",
            record.event, from, expected, record.state
        )));
    }
    match (&record.state, record.signed_order_ref.as_ref()) {
        (SignOnlyLifecycleState::SignedDryRun, Some(value)) if !value.trim().is_empty() => {}
        (SignOnlyLifecycleState::SignedDryRun, _) => {
            return Err(StoreError::Conflict(
                "SignedDryRun sign-only lifecycle record requires a non-empty signed_order_ref"
                    .into(),
            ));
        }
        (_, Some(_)) => {
            return Err(StoreError::Conflict(
                "signed_order_ref is only allowed for SignedDryRun sign-only lifecycle records"
                    .into(),
            ));
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn order_lifecycle_state_to_str(state: &OrderLifecycleState) -> &'static str {
    match state {
        OrderLifecycleState::Planned => "PLANNED",
        OrderLifecycleState::Signed => "SIGNED",
        OrderLifecycleState::PostRequested => "POST_REQUESTED",
        OrderLifecycleState::Posted => "POSTED",
        OrderLifecycleState::PartiallyFilled => "PARTIALLY_FILLED",
        OrderLifecycleState::Filled => "FILLED",
        OrderLifecycleState::CancelRequested => "CANCEL_REQUESTED",
        OrderLifecycleState::CancelRemoteAccepted => "CANCEL_REMOTE_ACCEPTED",
        OrderLifecycleState::CancelConfirmed => "CANCEL_CONFIRMED",
        OrderLifecycleState::RemoteUnknown => "REMOTE_UNKNOWN",
        OrderLifecycleState::PartialRemoteUnknown => "PARTIAL_REMOTE_UNKNOWN",
        OrderLifecycleState::Failed => "FAILED",
    }
}

pub(crate) fn order_lifecycle_state_from_str(
    value: &str,
) -> Result<OrderLifecycleState, StoreError> {
    match value {
        "PLANNED" => Ok(OrderLifecycleState::Planned),
        "SIGNED" => Ok(OrderLifecycleState::Signed),
        "POST_REQUESTED" => Ok(OrderLifecycleState::PostRequested),
        "POSTED" => Ok(OrderLifecycleState::Posted),
        "PARTIALLY_FILLED" => Ok(OrderLifecycleState::PartiallyFilled),
        "FILLED" => Ok(OrderLifecycleState::Filled),
        "CANCEL_REQUESTED" => Ok(OrderLifecycleState::CancelRequested),
        "CANCEL_REMOTE_ACCEPTED" => Ok(OrderLifecycleState::CancelRemoteAccepted),
        "CANCEL_CONFIRMED" => Ok(OrderLifecycleState::CancelConfirmed),
        "REMOTE_UNKNOWN" => Ok(OrderLifecycleState::RemoteUnknown),
        "PARTIAL_REMOTE_UNKNOWN" => Ok(OrderLifecycleState::PartialRemoteUnknown),
        "FAILED" => Ok(OrderLifecycleState::Failed),
        other => Err(StoreError::InvalidData(format!(
            "unknown order lifecycle state: {other}"
        ))),
    }
}

pub(crate) fn order_event_kind_to_str(event: &OrderEventKind) -> &'static str {
    match event {
        OrderEventKind::Signed => "SIGNED",
        OrderEventKind::PostRequested => "POST_REQUESTED",
        OrderEventKind::RemotePosted => "REMOTE_POSTED",
        OrderEventKind::RemoteRejected => "REMOTE_REJECTED",
        OrderEventKind::RemoteUnknown => "REMOTE_UNKNOWN",
        OrderEventKind::PartialFill => "PARTIAL_FILL",
        OrderEventKind::FullFill => "FULL_FILL",
        OrderEventKind::CancelRequested => "CANCEL_REQUESTED",
        OrderEventKind::CancelRemoteAccepted => "CANCEL_REMOTE_ACCEPTED",
        OrderEventKind::CancelConfirmed => "CANCEL_CONFIRMED",
        OrderEventKind::ReconcileOpen => "RECONCILE_OPEN",
        OrderEventKind::ReconcileMissing => "RECONCILE_MISSING",
    }
}

pub(crate) fn order_event_kind_from_str(value: &str) -> Result<OrderEventKind, StoreError> {
    match value {
        "SIGNED" => Ok(OrderEventKind::Signed),
        "POST_REQUESTED" => Ok(OrderEventKind::PostRequested),
        "REMOTE_POSTED" => Ok(OrderEventKind::RemotePosted),
        "REMOTE_REJECTED" => Ok(OrderEventKind::RemoteRejected),
        "REMOTE_UNKNOWN" => Ok(OrderEventKind::RemoteUnknown),
        "PARTIAL_FILL" => Ok(OrderEventKind::PartialFill),
        "FULL_FILL" => Ok(OrderEventKind::FullFill),
        "CANCEL_REQUESTED" => Ok(OrderEventKind::CancelRequested),
        "CANCEL_REMOTE_ACCEPTED" => Ok(OrderEventKind::CancelRemoteAccepted),
        "CANCEL_CONFIRMED" => Ok(OrderEventKind::CancelConfirmed),
        "RECONCILE_OPEN" => Ok(OrderEventKind::ReconcileOpen),
        "RECONCILE_MISSING" => Ok(OrderEventKind::ReconcileMissing),
        other => Err(StoreError::InvalidData(format!(
            "unknown order lifecycle event: {other}"
        ))),
    }
}

fn runtime_observation_worker_status(
    observations: &[RuntimeWorkerObservation],
) -> Option<WorkerStatus> {
    if observations.is_empty() {
        return None;
    }
    let mut has_degraded = false;
    let mut has_healthy = false;
    for observation in observations {
        let status = observation
            .status
            .trim()
            .to_ascii_uppercase()
            .replace('-', "_");
        if matches!(status.as_str(), "STALE" | "ERROR" | "DOWN") {
            return Some(WorkerStatus::Stale);
        }
        if matches!(status.as_str(), "UNKNOWN" | "UNOBSERVED") {
            return Some(WorkerStatus::Unknown);
        }
        if observation.should_fail_closed || matches!(status.as_str(), "DEGRADED" | "BLOCKED") {
            has_degraded = true;
        }
        if matches!(status.as_str(), "HEALTHY" | "OK" | "ALLOWED") {
            has_healthy = true;
        }
    }
    if has_degraded {
        Some(WorkerStatus::Degraded)
    } else if has_healthy {
        Some(WorkerStatus::Healthy)
    } else {
        Some(WorkerStatus::Unknown)
    }
}

pub fn apply_runtime_worker_observations(
    mut base: RuntimeStateSummary,
    observations: &[RuntimeWorkerObservation],
) -> RuntimeStateSummary {
    if let Some(observed_status) = runtime_observation_worker_status(observations) {
        base.worker_status = worst_worker_status(base.worker_status, observed_status);
        for observation in observations {
            if !base.required_capabilities.contains(&observation.capability) {
                base.required_capabilities
                    .push(observation.capability.clone());
            }
        }
    }
    base
}

fn worst_worker_status(left: WorkerStatus, right: WorkerStatus) -> WorkerStatus {
    use WorkerStatus::*;
    match (left, right) {
        (Stale, _) | (_, Stale) => Stale,
        (Unknown, _) | (_, Unknown) => Unknown,
        (Degraded, _) | (_, Degraded) => Degraded,
        (Healthy, Healthy) => Healthy,
    }
}

pub(crate) fn quantity_bound_to_resource_and_amount(
    bound: &QuantityBound,
) -> Result<(&'static str, &str), StoreError> {
    match bound {
        QuantityBound::WorstCaseQuoteNotional(v) => Ok(("worst_case_quote_notional", &v.0)),
        QuantityBound::WorstCaseBaseShares(v) => Ok(("worst_case_base_shares", &v.0)),
        QuantityBound::Unsupported(reason) => Err(StoreError::Conflict(format!(
            "unsupported quantity bound for reservation: {reason}"
        ))),
    }
}

pub(crate) fn reservation_state_to_str(state: &ReservationState) -> &'static str {
    match state {
        ReservationState::Pending => "PENDING",
        ReservationState::Active => "ACTIVE",
        ReservationState::Released => "RELEASED",
        ReservationState::Consumed => "CONSUMED",
        ReservationState::Orphaned => "ORPHANED",
    }
}

pub fn submit_status_str(status: &SubmitStatus) -> &'static str {
    match status {
        SubmitStatus::Accepted => "ACCEPTED",
        SubmitStatus::Posted => "POSTED",
        SubmitStatus::PartialRemoteUnknown => "PARTIAL_REMOTE_UNKNOWN",
        SubmitStatus::RemoteUnknown => "REMOTE_UNKNOWN",
        SubmitStatus::Rejected => "REJECTED",
        SubmitStatus::Blocked => "BLOCKED",
    }
}
