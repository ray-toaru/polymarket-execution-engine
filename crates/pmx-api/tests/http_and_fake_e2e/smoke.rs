use super::*;

#[tokio::test]
async fn http_auth_and_fake_e2e_smoke() {
    let _guard = env_lock().await;
    unsafe {
        std::env::set_var("PMX_API_SERVICE_TOKEN", "service-token-test");
        std::env::set_var("PMX_API_ADMIN_TOKEN", "admin-token-test");
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
        Some(json!({"scope": "ACCOUNT", "account_id": "acct-http-e2e-1", "enabled": true, "reason": "negative auth test"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = request_json(
        app.clone(),
        "GET",
        "/v1/admin/session",
        Some("service-token-test"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, admin_session) = request_json(
        app.clone(),
        "GET",
        "/v1/admin/session",
        Some("admin-token-test"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "admin session response: {admin_session}"
    );
    assert_eq!(admin_session["principal_subject"], "admin-token");
    assert_eq!(admin_session["scopes"], json!(["ADMIN"]));
    assert_eq!(
        admin_session["capabilities"],
        json!([
            "READ_AUDIT",
            "CANCEL_ORDER",
            "CANCEL_MARKET",
            "RECONCILE",
            "KILL_SWITCH"
        ])
    );
    assert_eq!(admin_session["no_remote_side_effect"], true);

    let (status, kill_switch) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/kill-switch",
        Some("admin-token-test"),
        Some(json!({"scope": "ACCOUNT", "account_id": "acct-http-e2e-1", "enabled": true, "reason": "admin auth test"})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "kill-switch response: {kill_switch}"
    );
    assert_eq!(kill_switch["enabled"], true);
    assert_eq!(kill_switch["scope"], "ACCOUNT");
    assert_eq!(kill_switch["account_id"], "acct-http-e2e-1");
    assert_eq!(kill_switch["persisted"], true);
    assert_eq!(kill_switch["state_version"], 1);

    let (status, global_kill_switch) = request_json(
        app,
        "POST",
        "/v1/admin/kill-switch",
        Some("admin-token-test"),
        Some(json!({"scope": "GLOBAL", "enabled": true, "reason": "global admin auth test"})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "global kill-switch response: {global_kill_switch}"
    );
    assert_eq!(global_kill_switch["enabled"], true);
    assert_eq!(global_kill_switch["scope"], "GLOBAL");
    assert!(global_kill_switch["account_id"].is_null());
    assert_eq!(global_kill_switch["persisted"], true);
    assert_eq!(global_kill_switch["state_version"], 1);
}
