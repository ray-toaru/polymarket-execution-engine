use super::*;

#[tokio::test]
async fn equal_service_and_admin_tokens_fail_closed_at_app_construction() {
    let _guard = env_lock().await;
    unsafe {
        std::env::set_var("PM_EXEC_SERVICE_TOKEN", "same-token-test");
        std::env::set_var("PM_EXEC_ADMIN_TOKEN", "same-token-test");
    }
    let err = pmx_api::try_app().expect_err("equal tokens must fail closed");
    assert!(err.contains("distinct"));
}

#[tokio::test]
async fn mismatched_object_graph_is_rejected() {
    let _guard = env_lock().await;
    unsafe {
        std::env::set_var("PM_EXEC_SERVICE_TOKEN", "service-token-mismatch");
        std::env::set_var("PM_EXEC_ADMIN_TOKEN", "admin-token-mismatch");
    }
    let app = pmx_api::app();
    let (status, normalized) = request_json(
        app.clone(),
        "POST",
        "/v1/intents/normalize",
        Some("service-token-mismatch"),
        Some(sample_intent()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, snapshot) = request_json(
        app.clone(),
        "POST",
        "/v1/snapshots/capture",
        Some("service-token-mismatch"),
        Some(normalized.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let mut second_intent = sample_intent();
    second_intent["client_intent_id"] = Value::String("intent-http-e2e-mismatch-2".into());
    second_intent["account_id"] = Value::String("acct-http-e2e-mismatch-2".into());
    let (status, second_normalized) = request_json(
        app.clone(),
        "POST",
        "/v1/intents/normalize",
        Some("service-token-mismatch"),
        Some(second_intent),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = request_json(
        app,
        "POST",
        "/v1/decisions/evaluate",
        Some("service-token-mismatch"),
        Some(json!({"normalized_intent_id": second_normalized["normalized_intent_id"], "snapshot_id": snapshot["snapshot_id"]})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}
