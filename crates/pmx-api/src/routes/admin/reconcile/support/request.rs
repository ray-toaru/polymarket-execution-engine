use super::*;

pub async fn require_reconcile_request(
    state: &AppState,
    headers: &HeaderMap,
    req: &ReconcileRequest,
) -> Result<ReconcileRequestParts, ReconcileApiError> {
    let (principal, correlation_id, fingerprint) =
        require_reconcile_context(state, headers, pmx_authz::Operation::Reconcile, req).await?;
    if req.reason.trim().is_empty() {
        return Err(reject_bad_request(
            state,
            &principal,
            "Reconcile",
            fingerprint,
            correlation_id,
            "reason must be non-empty",
        )
        .await?);
    }
    let local_reconcile = match (&req.order_id, &req.remote_observation) {
        (Some(order_id), Some(remote_observation)) => {
            Some((order_id.clone(), remote_observation.clone()))
        }
        (None, None) => None,
        _ => {
            return Err(reject_bad_request(
                state,
                &principal,
                "Reconcile",
                fingerprint,
                correlation_id,
                "order_id and remote_observation must be provided together",
            )
            .await?);
        }
    };
    Ok((principal, correlation_id, fingerprint, local_reconcile))
}
