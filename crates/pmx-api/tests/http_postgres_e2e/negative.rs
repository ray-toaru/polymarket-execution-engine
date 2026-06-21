use super::*;

#[tokio::test]
async fn http_postgres_rejects_cross_object_graph_and_bad_plan_hash() {
    let _guard = env_lock().await;
    let Ok(database_url) = std::env::var("PMX_TEST_DATABASE_URL") else {
        eprintln!("PMX_TEST_DATABASE_URL not set; skipping HTTP PostgreSQL negative E2E smoke");
        return;
    };
    unsafe {
        std::env::set_var("PMX_API_SERVICE_TOKEN", "service-token-pg-negative");
        std::env::set_var("PMX_API_ADMIN_TOKEN", "admin-token-pg-negative");
    }

    let app = pmx_api::try_postgres_app(database_url, true)
        .await
        .expect("postgres-backed app");

    let (status, normalized_a) = request_json(
        app.clone(),
        "POST",
        "/v1/intents/normalize",
        Some("service-token-pg-negative"),
        Some(sample_intent_variant("negative-a")),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "normalize A response: {normalized_a}"
    );

    let (status, snapshot_a) = request_json(
        app.clone(),
        "POST",
        "/v1/snapshots/capture",
        Some("service-token-pg-negative"),
        Some(normalized_a.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "snapshot A response: {snapshot_a}");

    let (status, normalized_b) = request_json(
        app.clone(),
        "POST",
        "/v1/intents/normalize",
        Some("service-token-pg-negative"),
        Some(sample_intent_variant("negative-b")),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "normalize B response: {normalized_b}"
    );

    let (status, mismatch) = request_json(
        app.clone(),
        "POST",
        "/v1/decisions/evaluate",
        Some("service-token-pg-negative"),
        Some(json!({
            "normalized_intent_id": normalized_b["normalized_intent_id"],
            "snapshot_id": snapshot_a["snapshot_id"]
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "mismatch response: {mismatch}"
    );

    let (status, decision_a) = request_json(
        app.clone(),
        "POST",
        "/v1/decisions/evaluate",
        Some("service-token-pg-negative"),
        Some(json!({
            "normalized_intent_id": normalized_a["normalized_intent_id"],
            "snapshot_id": snapshot_a["snapshot_id"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "decision A response: {decision_a}");

    let approval = approval_json("approval-pg-negative-1", &snapshot_a, &decision_a);
    let (status, plan) = request_json(
        app.clone(),
        "POST",
        "/v1/plans/compile",
        Some("service-token-pg-negative"),
        Some(json!({
            "normalized_intent_id": normalized_a["normalized_intent_id"],
            "snapshot_id": snapshot_a["snapshot_id"],
            "decision_id": decision_a["decision_id"],
            "approval": approval
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "plan response: {plan}");

    let (status, bad_submit) = request_json(
        app,
        "POST",
        "/v1/submissions",
        Some("service-token-pg-negative"),
        Some(json!({
            "execution_id": plan["execution_id"],
            "plan_hash": "wrong-plan-hash",
            "idempotency_key": "idem-pg-negative-1",
            "mode": "BLOCKED_DRY_RUN"
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "bad submit response: {bad_submit}"
    );
}
