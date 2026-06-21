use super::super::*;

pub(super) async fn compile_and_submit_blocked_plan(
    app: axum::Router,
    intent: Value,
    suffix: &str,
) -> (String, String) {
    let (status, health) = request_json(
        app.clone(),
        "GET",
        "/v1/health",
        Some("service-token-pg-e2e"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "health response: {health}");
    assert_eq!(health["checks"]["database"], "postgres");

    let (status, normalized) = request_json(
        app.clone(),
        "POST",
        "/v1/intents/normalize",
        Some("service-token-pg-e2e"),
        Some(intent),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "normalize response: {normalized}");

    let (status, snapshot) = request_json(
        app.clone(),
        "POST",
        "/v1/snapshots/capture",
        Some("service-token-pg-e2e"),
        Some(normalized.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "snapshot response: {snapshot}");

    let (status, decision) = request_json(
        app.clone(),
        "POST",
        "/v1/decisions/evaluate",
        Some("service-token-pg-e2e"),
        Some(json!({"normalized_intent_id": normalized["normalized_intent_id"], "snapshot_id": snapshot["snapshot_id"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "decision response: {decision}");

    let plan_normalized_id = normalized["normalized_intent_id"].clone();
    let plan_snapshot_id = snapshot["snapshot_id"].clone();
    let approval = approval_json(&format!("approval-pg-e2e-{suffix}"), &snapshot, &decision);
    let (status, plan) = request_json(
        app.clone(),
        "POST",
        "/v1/plans/compile",
        Some("service-token-pg-e2e"),
        Some(json!({
            "normalized_intent_id": plan_normalized_id,
            "snapshot_id": plan_snapshot_id,
            "decision_id": decision["decision_id"],
            "approval": approval
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "plan response: {plan}");

    let execution_id = plan["execution_id"]
        .as_str()
        .expect("execution_id")
        .to_owned();
    let plan_hash = plan["plan_hash"].as_str().expect("plan_hash").to_owned();
    let submit_body = json!({
        "execution_id": execution_id.clone(),
        "plan_hash": plan_hash.clone(),
        "idempotency_key": format!("idem-pg-e2e-{suffix}"),
        "mode": "BLOCKED_DRY_RUN"
    });
    let (status, first_submit) = request_json(
        app.clone(),
        "POST",
        "/v1/submissions",
        Some("service-token-pg-e2e"),
        Some(submit_body.clone()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "first submit response: {first_submit}"
    );
    assert_eq!(first_submit["status"], "BLOCKED");

    let (status, replay_submit) = request_json(
        app.clone(),
        "POST",
        "/v1/submissions",
        Some("service-token-pg-e2e"),
        Some(submit_body),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "replay submit response: {replay_submit}"
    );
    assert_eq!(first_submit, replay_submit);

    let submission_uri = format!(
        "/v1/submissions/{}",
        first_submit["execution_id"].as_str().unwrap()
    );
    let (status, loaded_submit) = request_json(
        app,
        "GET",
        &submission_uri,
        Some("service-token-pg-e2e"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "loaded submit response: {loaded_submit}"
    );
    assert_eq!(loaded_submit, first_submit);

    (execution_id, plan_hash)
}

#[tokio::test]
async fn compile_flow_propagates_header_correlation_id_across_object_graph() {
    let _guard = env_lock().await;
    let Ok(database_url) = std::env::var("PMX_TEST_DATABASE_URL") else {
        eprintln!(
            "PMX_TEST_DATABASE_URL not set; skipping compile correlation HTTP PostgreSQL E2E"
        );
        return;
    };
    unsafe {
        std::env::set_var("PMX_API_SERVICE_TOKEN", "service-token-pg-e2e");
        std::env::set_var("PMX_API_ADMIN_TOKEN", "admin-token-pg-e2e");
    }
    let suffix = unique_suffix("compile-correlation");
    let app = pmx_api::try_postgres_app(database_url.clone(), true)
        .await
        .expect("postgres app");
    seed_allow_runtime(
        &database_url,
        &format!("acct-http-pg-e2e-{suffix}"),
        &format!("cond-http-pg-e2e-{suffix}"),
        &suffix,
    )
    .await;

    let correlation_id = format!("corr-http-compile-{suffix}");
    let headers = [("X-Correlation-Id", correlation_id.as_str())];

    let (status, normalized) = request_json_with_headers(
        app.clone(),
        "POST",
        "/v1/intents/normalize",
        Some("service-token-pg-e2e"),
        Some(sample_intent_variant(&suffix)),
        &headers,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "normalize response: {normalized}");
    assert_eq!(normalized["correlation_id"], correlation_id);

    let (status, snapshot) = request_json_with_headers(
        app.clone(),
        "POST",
        "/v1/snapshots/capture",
        Some("service-token-pg-e2e"),
        Some(normalized.clone()),
        &headers,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "snapshot response: {snapshot}");
    assert_eq!(snapshot["correlation_id"], correlation_id);

    let (status, decision) = request_json_with_headers(
        app.clone(),
        "POST",
        "/v1/decisions/evaluate",
        Some("service-token-pg-e2e"),
        Some(json!({
            "normalized_intent_id": normalized["normalized_intent_id"],
            "snapshot_id": snapshot["snapshot_id"]
        })),
        &headers,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "decision response: {decision}");
    assert_eq!(decision["correlation_id"], correlation_id);

    let approval = approval_json(
        &format!("approval-pg-compile-corr-{suffix}"),
        &snapshot,
        &decision,
    );
    let (status, plan) = request_json_with_headers(
        app,
        "POST",
        "/v1/plans/compile",
        Some("service-token-pg-e2e"),
        Some(json!({
            "normalized_intent_id": normalized["normalized_intent_id"],
            "snapshot_id": snapshot["snapshot_id"],
            "decision_id": decision["decision_id"],
            "approval": approval
        })),
        &headers,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "plan response: {plan}");
    assert_eq!(plan["correlation_id"], correlation_id);
}

#[tokio::test]
async fn submit_plan_propagates_header_correlation_id_into_lifecycle_events() {
    let _guard = env_lock().await;
    let Ok(database_url) = std::env::var("PMX_TEST_DATABASE_URL") else {
        eprintln!("PMX_TEST_DATABASE_URL not set; skipping submit correlation HTTP PostgreSQL E2E");
        return;
    };
    unsafe {
        std::env::set_var("PMX_API_SERVICE_TOKEN", "service-token-pg-e2e");
        std::env::set_var("PMX_API_ADMIN_TOKEN", "admin-token-pg-e2e");
    }
    let suffix = unique_suffix("submit-correlation");
    let app = pmx_api::try_postgres_app(database_url.clone(), true)
        .await
        .expect("postgres app");
    seed_allow_runtime(
        &database_url,
        &format!("acct-http-pg-e2e-{suffix}"),
        &format!("cond-http-pg-e2e-{suffix}"),
        &suffix,
    )
    .await;

    let (execution_id, plan_hash) =
        compile_and_submit_blocked_plan(app.clone(), sample_intent_variant(&suffix), &suffix).await;
    let submit_body = json!({
        "execution_id": execution_id.clone(),
        "plan_hash": plan_hash,
        "idempotency_key": format!("idem-pg-corr-{suffix}"),
        "mode": "BLOCKED_DRY_RUN"
    });
    let correlation_id = format!("corr-http-pg-{suffix}");
    let (status, submit) = request_json_with_headers(
        app.clone(),
        "POST",
        "/v1/submissions",
        Some("service-token-pg-e2e"),
        Some(submit_body),
        &[("X-Correlation-Id", correlation_id.as_str())],
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "submit response: {submit}");

    let lifecycle_uri = format!("/v1/lifecycle/executions/{execution_id}/events?limit=10");
    let (status, lifecycle_events) = request_json(
        app,
        "GET",
        &lifecycle_uri,
        Some("service-token-pg-e2e"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "lifecycle events response: {lifecycle_events}"
    );
    let blocked = lifecycle_events
        .as_array()
        .expect("lifecycle events array")
        .iter()
        .find(|event| {
            event["event_type"] == "SUBMIT_BLOCKED_BEFORE_REMOTE"
                && event["payload"]["correlation_id"] == correlation_id
        })
        .expect("blocked lifecycle event");
    assert_eq!(blocked["payload"]["correlation_id"], correlation_id);
}
