use super::super::*;

pub(super) async fn verify_admin_cancel_and_reconcile(
    app: axum::Router,
    database_url: &str,
    execution_id: &str,
    suffix: &str,
) {
    let account_id = format!("acct-http-pg-e2e-{suffix}");
    let order_id = format!("order-http-pg-e2e-{suffix}");
    seed_cancelable_order(
        database_url,
        &account_id,
        &order_id,
        execution_id,
        &format!("cond-http-pg-e2e-{suffix}"),
        &format!("token-http-pg-e2e-{suffix}"),
    )
    .await;

    let (status, cancel) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/cancel-order",
        Some("admin-token-pg-e2e"),
        Some(json!({
            "account_id": account_id,
            "order_id": order_id,
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
