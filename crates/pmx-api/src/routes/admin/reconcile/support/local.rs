use super::*;

pub async fn require_local_reconcile_request(
    state: &AppState,
    headers: &HeaderMap,
    req: &crate::model::ReconcileOrderLocalRequest,
) -> Result<(pmx_authz::Principal, String, Option<String>), ReconcileApiError> {
    let (principal, correlation_id, fingerprint) =
        require_reconcile_context(state, headers, pmx_authz::Operation::Reconcile, req).await?;
    if req.account_id.trim().is_empty()
        || req.order_id.trim().is_empty()
        || req.reason.trim().is_empty()
    {
        return Err(reject_bad_request(
            state,
            &principal,
            "ReconcileOrderLocal",
            fingerprint,
            correlation_id,
            "account_id, order_id and reason must be non-empty",
        )
        .await?);
    }
    Ok((principal, correlation_id, fingerprint))
}
