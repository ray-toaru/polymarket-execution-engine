use super::*;

#[tokio::test]
async fn http_postgres_admin_routes_record_audit_events() {
    let _guard = env_lock().await;
    let Ok(database_url) = std::env::var("PMX_TEST_DATABASE_URL") else {
        eprintln!("PMX_TEST_DATABASE_URL not set; skipping HTTP PostgreSQL admin audit E2E smoke");
        return;
    };
    unsafe {
        std::env::set_var("PM_EXEC_SERVICE_TOKEN", "service-token-pg-audit");
        std::env::set_var("PM_EXEC_ADMIN_TOKEN", "admin-token-pg-audit");
    }

    let app = pmx_api::try_postgres_app(database_url.clone(), true)
        .await
        .expect("postgres-backed app");

    let (status, receipt) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/kill-switch",
        Some("admin-token-pg-audit"),
        Some(json!({"enabled": true, "reason": "audit e2e"})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "kill-switch response: {receipt}"
    );

    let (status, cancel) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/cancel-order",
        Some("admin-token-pg-audit"),
        Some(json!({"account_id": "acct-audit", "order_id": "order-audit", "reason": "audit e2e"})),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "cancel response: {cancel}");

    let (status, rejected) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/cancel-order",
        Some("admin-token-pg-audit"),
        Some(json!({"account_id": "acct-audit", "order_id": "order-audit-rejected", "reason": ""})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "rejected cancel response: {rejected}"
    );

    let (client, connection) = tokio_postgres::connect(&database_url, tokio_postgres::NoTls)
        .await
        .expect("connect for audit count");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let row = client
        .query_one(
            "SELECT COUNT(*)::bigint FROM admin_audit_events WHERE principal_subject = 'admin-token' AND operation IN ('KillSwitch', 'CancelOrder')",
            &[],
        )
        .await
        .expect("count audit events");
    let count: i64 = row.get(0);
    assert!(
        count >= 2,
        "expected at least two admin audit events, got {count}"
    );
    let rejected_row = client
        .query_one(
            "SELECT COUNT(*)::bigint FROM admin_audit_events WHERE principal_subject = 'admin-token' AND operation = 'CancelOrder' AND result LIKE 'REJECTED%'",
            &[],
        )
        .await
        .expect("count rejected audit events");
    let rejected_count: i64 = rejected_row.get(0);
    assert!(rejected_count >= 1, "expected rejected cancel audit event");

    let (status, _) = request_json(
        app.clone(),
        "GET",
        "/v1/admin/audit-events?limit=20",
        Some("service-token-pg-audit"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, audit_events) = request_json(
        app,
        "GET",
        "/v1/admin/audit-events?limit=20",
        Some("admin-token-pg-audit"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "audit query response: {audit_events}"
    );
    assert!(audit_events.as_array().unwrap().len() >= 2);
}
