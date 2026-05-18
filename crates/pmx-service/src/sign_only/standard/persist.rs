use pmx_core::{
    AccountId, ExecutionId, SignOnlyLifecycleEventKind, SignOnlyLifecycleRecord,
    SignOnlyLifecycleState,
};

pub(super) fn standard_stages() -> [(
    SignOnlyLifecycleEventKind,
    SignOnlyLifecycleState,
    Option<&'static str>,
    &'static str,
); 3] {
    [
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
            Some("signed"),
            "signed-without-post",
        ),
    ]
}

pub(super) fn try_replay_standard_sign_only(
    existing: &[SignOnlyLifecycleRecord],
    plan_hash: &str,
) -> Option<Vec<SignOnlyLifecycleRecord>> {
    let expected_client_event_ids: Vec<String> = standard_stages()
        .iter()
        .map(|(_, _, _, stage)| format!("sdk-standard:{plan_hash}:{stage}"))
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
    (replay_records.len() == standard_stages().len()).then_some(replay_records)
}

pub(super) fn build_standard_sign_only_records(
    execution_id: &str,
    account_id: &str,
    plan_hash: &str,
    signed_order_ref: &str,
) -> Vec<SignOnlyLifecycleRecord> {
    standard_stages()
        .into_iter()
        .map(
            |(event, state, signed_stage, stage)| SignOnlyLifecycleRecord {
                execution_id: ExecutionId(execution_id.to_string()),
                account_id: AccountId(account_id.to_string()),
                state,
                event,
                client_event_id: Some(format!("sdk-standard:{plan_hash}:{stage}")),
                signed_order_ref: signed_stage.map(|_| signed_order_ref.to_string()),
                no_remote_side_effect: true,
                event_id: None,
                created_at: None,
            },
        )
        .collect()
}
