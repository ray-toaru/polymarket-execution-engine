use super::super::*;

pub(super) async fn verify_ready_plan_and_blocked_submit(
    app: axum::Router,
    database_url: &str,
    suffix: &str,
    normalized: Value,
    snapshot: Value,
    decision: Value,
) {
    let approval = approval_json(
        &format!("approval-pg-runtime-{suffix}"),
        &snapshot,
        &decision,
    );
    let (status, plan) = request_json(
        app.clone(),
        "POST",
        "/v1/plans/compile",
        Some("service-token-pg-runtime"),
        Some(json!({
            "normalized_intent_id": normalized["normalized_intent_id"],
            "snapshot_id": snapshot["snapshot_id"],
            "decision_id": decision["decision_id"],
            "approval": approval
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "plan response: {plan}");
    assert_eq!(plan["status"], "READY");

    let (status, submit) = request_json(
        app,
        "POST",
        "/v1/submissions",
        Some("service-token-pg-runtime"),
        Some(json!({
            "execution_id": plan["execution_id"],
            "plan_hash": plan["plan_hash"],
            "idempotency_key": format!("idem-pg-runtime-{suffix}"),
            "mode": "BLOCKED_DRY_RUN"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "submit response: {submit}");
    assert_eq!(submit["status"], "BLOCKED");

    let (client, connection) = tokio_postgres::connect(database_url, tokio_postgres::NoTls)
        .await
        .expect("connect for lifecycle count");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let execution_id = submit["execution_id"]
        .as_str()
        .expect("execution id")
        .to_owned();
    let row = client
        .query_one(
            "SELECT COUNT(*)::bigint FROM execution_lifecycle_events WHERE execution_id = $1 AND event_type = 'SUBMIT_BLOCKED_BEFORE_REMOTE'",
            &[&execution_id],
        )
        .await
        .expect("count lifecycle events");
    let count: i64 = row.get(0);
    assert!(count >= 1, "expected blocked-submit lifecycle event");
}
