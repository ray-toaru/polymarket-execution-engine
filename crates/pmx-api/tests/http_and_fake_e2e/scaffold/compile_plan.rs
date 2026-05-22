use super::super::*;

pub(super) async fn compile_blocked_plan(app: axum::Router) -> (String, String) {
    let (status, normalized) = request_json(
        app.clone(),
        "POST",
        "/v1/intents/normalize",
        Some("service-token-test-v07"),
        Some(sample_intent()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, snapshot) = request_json(
        app.clone(),
        "POST",
        "/v1/snapshots/capture",
        Some("service-token-test-v07"),
        Some(normalized.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, decision) = request_json(
        app.clone(),
        "POST",
        "/v1/decisions/evaluate",
        Some("service-token-test-v07"),
        Some(json!({"normalized_intent_id": normalized["normalized_intent_id"], "snapshot_id": snapshot["snapshot_id"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, plan) = request_json(
        app,
        "POST",
        "/v1/plans/compile",
        Some("service-token-test-v07"),
        Some(json!({
            "normalized_intent_id": normalized["normalized_intent_id"],
            "snapshot_id": snapshot["snapshot_id"],
            "decision_id": decision["decision_id"],
            "approval": approval_json("approval-v07-1", &snapshot, &decision)
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "plan response: {plan}");
    assert_eq!(plan["status"], "BLOCKED");

    (
        plan["execution_id"]
            .as_str()
            .expect("execution_id")
            .to_string(),
        plan["plan_hash"].as_str().expect("plan_hash").to_string(),
    )
}
