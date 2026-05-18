use pmx_core::{
    AccountId, ExecutionId, SignOnlyLifecycleEventKind, SignOnlyLifecycleRecord,
    SignOnlyLifecycleState, sign_only_lifecycle_records_equivalent,
};
use pmx_store::{ExecutionStore, SignOnlyLifecycleQuery, SignOnlyLifecycleStore};

use crate::binding::validate_sign_only_lifecycle_append;
use crate::{
    ServiceError, StandardSignOnlyConstructionReceipt, StandardSignOnlyConstructionRequest,
};

pub async fn record_sign_only_lifecycle_event<S>(
    store: &S,
    mut record: SignOnlyLifecycleRecord,
) -> Result<SignOnlyLifecycleRecord, ServiceError>
where
    S: SignOnlyLifecycleStore + Send + Sync,
{
    record.event_id = None;
    record.created_at = None;
    let query = SignOnlyLifecycleQuery {
        execution_id: record.execution_id.0.clone(),
        limit: 500,
        before_event_id: None,
    };
    let existing = store.list_sign_only_lifecycle_events(&query).await?;
    validate_sign_only_lifecycle_append(&existing, &record)?;
    store.record_sign_only_lifecycle_event(&record).await?;
    let updated = store.list_sign_only_lifecycle_events(&query).await?;
    let matched = if let Some(client_event_id) = record.client_event_id.as_deref() {
        updated
            .iter()
            .rev()
            .find(|candidate| candidate.client_event_id.as_deref() == Some(client_event_id))
    } else {
        updated
            .iter()
            .rev()
            .find(|candidate| sign_only_lifecycle_records_equivalent(candidate, &record))
    };
    Ok(matched.cloned().unwrap_or(record))
}

pub async fn record_standard_sign_only_construction<S>(
    store: &S,
    req: StandardSignOnlyConstructionRequest,
) -> Result<StandardSignOnlyConstructionReceipt, ServiceError>
where
    S: ExecutionStore + SignOnlyLifecycleStore + Send + Sync,
{
    if req.execution_id.trim().is_empty()
        || req.account_id.trim().is_empty()
        || req.plan_hash.trim().is_empty()
        || req.signed_order_ref.trim().is_empty()
    {
        return Err(ServiceError::BadRequest(
            "execution_id, account_id, plan_hash and signed_order_ref must be non-empty".into(),
        ));
    }
    if !req.no_remote_side_effect {
        return Err(ServiceError::BadRequest(
            "standard sign-only construction must not contain remote side effects".into(),
        ));
    }
    if !req.signed_order_ref.starts_with("sign-only:") {
        return Err(ServiceError::BadRequest(
            "standard sign-only construction requires a redacted sign-only ref".into(),
        ));
    }
    if let Some(digest) = req.signed_order_digest.as_deref()
        && (digest.len() != 64 || !digest.chars().all(|ch| ch.is_ascii_hexdigit()))
    {
        return Err(ServiceError::BadRequest(
            "signed_order_digest must be a 64-character hex SHA-256 digest".into(),
        ));
    }
    let plan = store.load_plan_summary(&req.execution_id).await?;
    if plan.account_id.0 != req.account_id {
        return Err(ServiceError::Conflict(
            "sign-only construction account_id does not match execution plan".into(),
        ));
    }
    if plan.plan_hash.0 != req.plan_hash {
        return Err(ServiceError::Conflict(
            "sign-only construction plan_hash does not match execution plan".into(),
        ));
    }

    let stages = [
        (
            SignOnlyLifecycleEventKind::PrepareReservation,
            SignOnlyLifecycleState::ReservationPrepared,
            None,
            "prepare-reservation",
        ),
        (
            SignOnlyLifecycleEventKind::RequestSigning,
            SignOnlyLifecycleState::SigningRequested,
            None,
            "request-signing",
        ),
        (
            SignOnlyLifecycleEventKind::SignedWithoutPost,
            SignOnlyLifecycleState::SignedDryRun,
            Some(req.signed_order_ref.clone()),
            "signed-without-post",
        ),
    ];
    let mut lifecycle_records = Vec::with_capacity(stages.len());
    for (event, state, signed_order_ref, stage) in stages {
        let record = record_sign_only_lifecycle_event(
            store,
            SignOnlyLifecycleRecord {
                execution_id: ExecutionId(req.execution_id.clone()),
                account_id: AccountId(req.account_id.clone()),
                state,
                event,
                client_event_id: Some(format!("sdk-standard:{}:{stage}", req.plan_hash)),
                signed_order_ref,
                no_remote_side_effect: true,
                event_id: None,
                created_at: None,
            },
        )
        .await?;
        lifecycle_records.push(record);
    }

    Ok(StandardSignOnlyConstructionReceipt {
        execution_id: req.execution_id,
        signed_order_ref: req.signed_order_ref,
        signed_order_digest: req.signed_order_digest,
        lifecycle_records,
        no_remote_side_effect: true,
    })
}
