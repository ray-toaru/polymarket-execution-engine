use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{AccountId, CoreError, ExecutionId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignOnlyLifecycleState {
    Planned,
    ReservationPrepared,
    SigningRequested,
    SignedDryRun,
    Failed,
    Abandoned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignOnlyLifecycleEventKind {
    PrepareReservation,
    RequestSigning,
    SignedWithoutPost,
    SigningFailed,
    Abandon,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignOnlyLifecycleRecord {
    pub execution_id: ExecutionId,
    pub account_id: AccountId,
    pub state: SignOnlyLifecycleState,
    pub event: SignOnlyLifecycleEventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_event_id: Option<String>,
    pub signed_order_ref: Option<String>,
    pub no_remote_side_effect: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

pub fn sign_only_lifecycle_records_equivalent(
    left: &SignOnlyLifecycleRecord,
    right: &SignOnlyLifecycleRecord,
) -> bool {
    left.execution_id == right.execution_id
        && left.account_id == right.account_id
        && left.state == right.state
        && left.event == right.event
        && left.client_event_id == right.client_event_id
        && left.signed_order_ref == right.signed_order_ref
        && left.no_remote_side_effect == right.no_remote_side_effect
}

pub fn transition_sign_only_lifecycle(
    from: SignOnlyLifecycleState,
    event: SignOnlyLifecycleEventKind,
) -> Result<SignOnlyLifecycleState, CoreError> {
    let next = match (&from, &event) {
        (SignOnlyLifecycleState::Planned, SignOnlyLifecycleEventKind::PrepareReservation) => {
            SignOnlyLifecycleState::ReservationPrepared
        }
        (
            SignOnlyLifecycleState::ReservationPrepared,
            SignOnlyLifecycleEventKind::RequestSigning,
        ) => SignOnlyLifecycleState::SigningRequested,
        (
            SignOnlyLifecycleState::SigningRequested,
            SignOnlyLifecycleEventKind::SignedWithoutPost,
        ) => SignOnlyLifecycleState::SignedDryRun,
        (SignOnlyLifecycleState::SigningRequested, SignOnlyLifecycleEventKind::SigningFailed)
        | (
            SignOnlyLifecycleState::ReservationPrepared,
            SignOnlyLifecycleEventKind::SigningFailed,
        ) => SignOnlyLifecycleState::Failed,
        (SignOnlyLifecycleState::Planned, SignOnlyLifecycleEventKind::Abandon)
        | (SignOnlyLifecycleState::ReservationPrepared, SignOnlyLifecycleEventKind::Abandon)
        | (SignOnlyLifecycleState::SigningRequested, SignOnlyLifecycleEventKind::Abandon) => {
            SignOnlyLifecycleState::Abandoned
        }
        _ => return Err(CoreError::InvalidSignOnlyTransition { from, event }),
    };
    Ok(next)
}

pub fn sign_only_lifecycle_has_remote_side_effect(record: &SignOnlyLifecycleRecord) -> bool {
    !record.no_remote_side_effect
}
