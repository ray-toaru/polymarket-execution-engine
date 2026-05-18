use pmx_core::{
    SignOnlyLifecycleRecord, SignOnlyLifecycleState, sign_only_lifecycle_records_equivalent,
    transition_sign_only_lifecycle,
};

use crate::StoreError;

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
