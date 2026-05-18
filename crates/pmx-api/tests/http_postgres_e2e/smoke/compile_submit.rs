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
    let approval = json!({
        "approval_id": format!("approval-pg-e2e-{suffix}"),
        "approved_by": "operator-pg-e2e",
        "approved_at": "2026-05-15T00:00:00Z",
        "approval_hash": format!("approval-hash-pg-e2e-{suffix}")
    });
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
        "idempotency_key": format!("idem-pg-e2e-{suffix}")
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
