use super::*;

#[tokio::test]
async fn http_auth_and_fake_e2e_smoke() {
    unsafe {
        std::env::set_var("PM_EXEC_SERVICE_TOKEN", "service-token-test");
        std::env::set_var("PM_EXEC_ADMIN_TOKEN", "admin-token-test");
    }

    let app = pmx_api::app();

    let (status, _) = request_json(app.clone(), "GET", "/v1/health", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = request_json(app.clone(), "GET", "/v1/health", Some("bad-token"), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, normalized) = request_json(
        app.clone(),
        "POST",
        "/v1/intents/normalize",
        Some("service-token-test"),
        Some(sample_intent()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "normalize response: {normalized}");
    assert_eq!(normalized["side"], "BUY");
    assert!(
        normalized["normalized_intent_id"]
            .as_str()
            .unwrap()
            .starts_with("norm-")
    );

    let (status, snapshot) = request_json(
        app.clone(),
        "POST",
        "/v1/snapshots/capture",
        Some("service-token-test"),
        Some(normalized.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "snapshot response: {snapshot}");

    let (status, decision) = request_json(
        app.clone(),
        "POST",
        "/v1/decisions/evaluate",
        Some("service-token-test"),
        Some(json!({"normalized_intent_id": normalized["normalized_intent_id"], "snapshot_id": snapshot["snapshot_id"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "decision response: {decision}");
    assert_eq!(decision["status"], "BLOCK");

    let (status, _) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/kill-switch",
        Some("service-token-test"),
        Some(json!({"enabled": true, "reason": "negative auth test"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, kill_switch) = request_json(
        app,
        "POST",
        "/v1/admin/kill-switch",
        Some("admin-token-test"),
        Some(json!({"enabled": true, "reason": "admin auth test"})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "kill-switch response: {kill_switch}"
    );
    assert_eq!(kill_switch["enabled"], true);
}
