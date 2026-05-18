use crate::{OfficialSdkAdapterError, SignOnlyDryRunReceipt};
use pmx_core::{
    SignOnlyLifecycleEventKind, SignOnlyLifecycleRecord, SignOnlyLifecycleState,
    transition_sign_only_lifecycle,
};

/// Build a conservative sign-only lifecycle trace that can be persisted by the executor.
///
/// The trace deliberately terminates at `SignedDryRun`. It is invalid for this helper to
/// accept a receipt that claims it was posted, because sign-only dry-runs are non-mutating
/// probes and must not create remote Polymarket side effects.
pub fn sign_only_lifecycle_records_from_receipt(
    receipt: &SignOnlyDryRunReceipt,
) -> Result<Vec<SignOnlyLifecycleRecord>, OfficialSdkAdapterError> {
    if receipt.posted {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "sign-only receipt unexpectedly indicates remote posting".into(),
        ));
    }

    let s1 = transition_sign_only_lifecycle(
        SignOnlyLifecycleState::Planned,
        SignOnlyLifecycleEventKind::PrepareReservation,
    )
    .map_err(|err| OfficialSdkAdapterError::InvalidInput(err.to_string()))?;
    let s2 = transition_sign_only_lifecycle(s1.clone(), SignOnlyLifecycleEventKind::RequestSigning)
        .map_err(|err| OfficialSdkAdapterError::InvalidInput(err.to_string()))?;
    let s3 =
        transition_sign_only_lifecycle(s2.clone(), SignOnlyLifecycleEventKind::SignedWithoutPost)
            .map_err(|err| OfficialSdkAdapterError::InvalidInput(err.to_string()))?;

    Ok(vec![
        SignOnlyLifecycleRecord {
            execution_id: receipt.execution_id.clone(),
            account_id: receipt.account_id.clone(),
            state: s1,
            event: SignOnlyLifecycleEventKind::PrepareReservation,
            client_event_id: None,
            signed_order_ref: None,
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        },
        SignOnlyLifecycleRecord {
            execution_id: receipt.execution_id.clone(),
            account_id: receipt.account_id.clone(),
            state: s2,
            event: SignOnlyLifecycleEventKind::RequestSigning,
            client_event_id: None,
            signed_order_ref: None,
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        },
        SignOnlyLifecycleRecord {
            execution_id: receipt.execution_id.clone(),
            account_id: receipt.account_id.clone(),
            state: s3,
            event: SignOnlyLifecycleEventKind::SignedWithoutPost,
            client_event_id: None,
            signed_order_ref: Some(receipt.signed_order_ref.clone()),
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        },
    ])
}
