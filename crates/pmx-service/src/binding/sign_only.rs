use super::*;

pub(crate) fn validate_sign_only_lifecycle_append(
    existing: &[SignOnlyLifecycleRecord],
    record: &SignOnlyLifecycleRecord,
) -> Result<(), ServiceError> {
    if !record.no_remote_side_effect {
        return Err(ServiceError::BadRequest(
            "sign-only lifecycle record must not contain remote side effects".into(),
        ));
    }
    if existing
        .last()
        .map(|last| sign_only_lifecycle_records_equivalent(last, record))
        .unwrap_or(false)
    {
        return Ok(());
    }
    if let Some(first) = existing.first()
        && first.account_id != record.account_id
    {
        return Err(ServiceError::Conflict(
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
        return Err(ServiceError::Conflict(
            "sign-only lifecycle is already terminal".into(),
        ));
    }
    let expected = transition_sign_only_lifecycle(from.clone(), record.event.clone())
        .map_err(|err| ServiceError::Conflict(err.to_string()))?;
    if expected != record.state {
        return Err(ServiceError::Conflict(format!(
            "sign-only lifecycle state mismatch: event {:?} from {:?} yields {:?}, got {:?}",
            record.event, from, expected, record.state
        )));
    }
    match (&record.state, record.signed_order_ref.as_ref()) {
        (SignOnlyLifecycleState::SignedDryRun, Some(value)) if !value.trim().is_empty() => {}
        (SignOnlyLifecycleState::SignedDryRun, _) => {
            return Err(ServiceError::BadRequest(
                "SignedDryRun sign-only lifecycle record requires a non-empty signed_order_ref"
                    .into(),
            ));
        }
        (_, Some(_)) => {
            return Err(ServiceError::BadRequest(
                "signed_order_ref is only allowed for SignedDryRun sign-only lifecycle records"
                    .into(),
            ));
        }
        _ => {}
    }
    Ok(())
}
