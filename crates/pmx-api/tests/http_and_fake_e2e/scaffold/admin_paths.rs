use super::super::*;

pub(super) async fn verify_non_live_admin_paths(app: axum::Router, execution_id: &str) {
    let (status, _) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/cancel-order",
        Some("service-token-test-v07"),
        Some(json!({"account_id": "acct-http-e2e-1", "order_id": "order-v07-1", "execution_id": execution_id, "reason": "service must not cancel"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, cancel) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/cancel-order",
        Some("admin-token-test-v07"),
        Some(json!({"account_id": "acct-http-e2e-1", "order_id": "order-v07-1", "execution_id": execution_id, "reason": "admin cancel smoke"})),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "cancel response: {cancel}");

    let (status, reconcile) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/reconcile",
        Some("admin-token-test-v07"),
        Some(json!({"account_id": "acct-http-e2e-1", "execution_id": execution_id, "reason": "admin reconcile smoke"})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "reconcile response: {reconcile}"
    );
    assert_eq!(reconcile["checked_orders"], 0);

    let (status, reconcile_bad_pair) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/reconcile",
        Some("admin-token-test-v07"),
        Some(json!({
            "account_id": "acct-http-e2e-1",
            "execution_id": execution_id,
            "order_id": "order-v07-1",
            "reason": "admin reconcile bad pair"
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "reconcile bad pair response: {reconcile_bad_pair}"
    );

    let (status, reconcile_missing_order) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/reconcile",
        Some("admin-token-test-v07"),
        Some(json!({
            "account_id": "acct-http-e2e-1",
            "execution_id": execution_id,
            "order_id": "missing-order-v24-public",
            "remote_observation": "MISSING",
            "reason": "admin reconcile public local missing"
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "reconcile missing order response: {reconcile_missing_order}"
    );

    let (status, local_reconcile_missing) = request_json(
        app,
        "POST",
        "/v1/admin/reconcile-order-local",
        Some("admin-token-test-v07"),
        Some(json!({
            "account_id": "acct-http-e2e-1",
            "order_id": "missing-order-v24-1",
            "remote_observation": "MISSING",
            "reason": "admin local reconcile missing smoke"
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "local reconcile missing response: {local_reconcile_missing}"
    );
    assert!(local_reconcile_missing["correlation_id"].as_str().is_some());
}
