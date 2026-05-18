use pmx_core::{
    AccountId, ExecutionId, SignOnlyLifecycleEventKind, SignOnlyLifecycleRecord,
    SignOnlyLifecycleState, canonical_json_sha256,
};
use pmx_store::{ExecutionStore, SignOnlyLifecycleQuery, SignOnlyLifecycleStore};

use crate::sign_only::record_sign_only_lifecycle_event;
use crate::{
    ServiceError, StandardSignOnlyConstructionReceipt, StandardSignOnlyConstructionRequest,
};

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
    {
        return Err(ServiceError::BadRequest(
            "execution_id, account_id and plan_hash must be non-empty".into(),
        ));
    }
    if !req.no_remote_side_effect {
        return Err(ServiceError::BadRequest(
            "standard sign-only construction must not contain remote side effects".into(),
        ));
    }
    if let Some(signed_order_ref) = req.signed_order_ref.as_deref()
        && !signed_order_ref.starts_with("sign-only:")
    {
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
    let signed_order_digest = match req.signed_order_digest {
        Some(digest) => Some(digest),
        None => Some(derive_standard_sign_only_digest(&plan)?),
    };
    let signed_order_ref = req.signed_order_ref.unwrap_or_else(|| {
        let digest = signed_order_digest.as_deref().unwrap_or_default();
        format!(
            "sign-only:{}:{}:digest-{}",
            req.execution_id,
            req.plan_hash,
            &digest[..16]
        )
    });

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
            Some(signed_order_ref.clone()),
            "signed-without-post",
        ),
    ];
    let query = SignOnlyLifecycleQuery {
        execution_id: req.execution_id.clone(),
        limit: 500,
        before_event_id: None,
    };
    let existing = store.list_sign_only_lifecycle_events(&query).await?;
    let expected_client_event_ids: Vec<String> = stages
        .iter()
        .map(|(_, _, _, stage)| format!("sdk-standard:{}:{stage}", req.plan_hash))
        .collect();
    let replay_records: Vec<SignOnlyLifecycleRecord> = existing
        .iter()
        .filter(|record| {
            record
                .client_event_id
                .as_ref()
                .is_some_and(|id| expected_client_event_ids.contains(id))
        })
        .cloned()
        .collect();
    if replay_records.len() == stages.len() {
        return Ok(StandardSignOnlyConstructionReceipt {
            execution_id: req.execution_id,
            signed_order_ref,
            signed_order_digest,
            lifecycle_records: replay_records,
            no_remote_side_effect: true,
        });
    }

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
        signed_order_ref,
        signed_order_digest,
        lifecycle_records,
        no_remote_side_effect: true,
    })
}

fn derive_standard_sign_only_digest(
    plan: &pmx_core::ExecutionPlanSummary,
) -> Result<String, ServiceError> {
    let payload = serde_json::json!({
        "schema_version": 1,
        "construction_source": "official-sdk-standard-sign-only",
        "execution_id": plan.execution_id,
        "account_id": plan.account_id,
        "plan_hash": plan.plan_hash,
        "profile": {
            "signed_order_ref_namespace": "sign-only",
            "exposes_raw_signed_order": false,
            "may_post_order": false,
            "may_cancel_order": false,
            "no_remote_side_effect": true
        }
    });
    canonical_json_sha256(&payload)
        .map(|hash| hash.0)
        .map_err(|err| ServiceError::Internal(err.to_string()))
}
