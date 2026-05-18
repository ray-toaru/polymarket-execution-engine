use super::super::*;

pub(super) async fn verify_admin_cancel_and_reconcile(
    app: axum::Router,
    execution_id: &str,
    suffix: &str,
) {
    let (status, cancel) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/cancel-order",
        Some("admin-token-pg-e2e"),
        Some(json!({
            "account_id": format!("acct-http-pg-e2e-{suffix}"),
            "order_id": format!("order-pg-e2e-{suffix}"),
            "execution_id": execution_id,
            "reason": "pg cancel lifecycle smoke"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "cancel response: {cancel}");

    let (status, reconcile) = request_json(
        app,
        "POST",
        "/v1/admin/reconcile",
        Some("admin-token-pg-e2e"),
        Some(json!({
            "account_id": format!("acct-http-pg-e2e-{suffix}"),
            "execution_id": execution_id,
            "reason": "pg reconcile lifecycle smoke"
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "reconcile response: {reconcile}"
    );
}
