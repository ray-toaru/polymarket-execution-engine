use super::*;

#[tokio::test]
async fn http_postgres_runtime_rows_can_reach_ready_plan_but_submit_still_blocks() {
    let Ok(database_url) = std::env::var("PMX_TEST_DATABASE_URL") else {
        eprintln!("PMX_TEST_DATABASE_URL not set; skipping HTTP PostgreSQL runtime E2E smoke");
        return;
    };
    unsafe {
        std::env::set_var("PM_EXEC_SERVICE_TOKEN", "service-token-pg-runtime");
        std::env::set_var("PM_EXEC_ADMIN_TOKEN", "admin-token-pg-runtime");
    }
    let suffix = unique_suffix("runtime-allow");
    let app = pmx_api::try_postgres_app(database_url.clone(), true)
        .await
        .expect("postgres-backed app");
    let intent = sample_intent_variant(&suffix);
    let account_id = intent["account_id"]
        .as_str()
        .expect("account id")
        .to_owned();
    let condition_id = intent["market"]["condition_id"]
        .as_str()
        .expect("condition id")
        .to_owned();
    seed_allow_runtime(&database_url, &account_id, &condition_id, &suffix).await;

    let (status, normalized) = request_json(
        app.clone(),
        "POST",
        "/v1/intents/normalize",
        Some("service-token-pg-runtime"),
        Some(intent),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "normalize response: {normalized}");

    let (status, snapshot) = request_json(
        app.clone(),
        "POST",
        "/v1/snapshots/capture",
        Some("service-token-pg-runtime"),
        Some(normalized.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "snapshot response: {snapshot}");
    assert_eq!(snapshot["runtime_state"]["geoblock_status"], "ALLOWED");
    assert_eq!(snapshot["runtime_state"]["worker_status"], "HEALTHY");

    seed_runtime_worker_observation(
        &database_url,
        &account_id,
        "heartbeat",
        "STALE",
        true,
        "heartbeat lease expired",
    )
    .await;
    let runtime_workers_uri = format!("/v1/runtime/workers?account_id={account_id}&limit=20");
    let (status, runtime_workers) = request_json(
        app.clone(),
        "GET",
        &runtime_workers_uri,
        Some("service-token-pg-runtime"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "runtime worker status response: {runtime_workers}"
    );
    assert!(
        runtime_workers["heartbeats"]
            .as_array()
            .unwrap()
            .iter()
            .any(|heartbeat| heartbeat["capability"] == "heartbeat")
    );
    assert!(
        runtime_workers["observations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|observation| observation["status"] == "STALE")
    );
    let (status, degraded_snapshot) = request_json(
        app.clone(),
        "POST",
        "/v1/snapshots/capture",
        Some("service-token-pg-runtime"),
        Some(normalized.clone()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "degraded snapshot response: {degraded_snapshot}"
    );
    assert_eq!(degraded_snapshot["runtime_state"]["worker_status"], "STALE");
    assert!(
        degraded_snapshot["runtime_state"]["required_capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "heartbeat")
    );

    let (status, degraded_decision) = request_json(
        app.clone(),
        "POST",
        "/v1/decisions/evaluate",
        Some("service-token-pg-runtime"),
        Some(json!({"normalized_intent_id": normalized["normalized_intent_id"], "snapshot_id": degraded_snapshot["snapshot_id"]})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "degraded decision response: {degraded_decision}"
    );
    assert_eq!(degraded_decision["status"], "BLOCK");

    let (status, decision) = request_json(
        app.clone(),
        "POST",
        "/v1/decisions/evaluate",
        Some("service-token-pg-runtime"),
        Some(json!({"normalized_intent_id": normalized["normalized_intent_id"], "snapshot_id": snapshot["snapshot_id"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "decision response: {decision}");
    assert_eq!(decision["status"], "ALLOW");

    let approval = json!({
        "approval_id": format!("approval-pg-runtime-{suffix}"),
        "approved_by": "operator-pg-runtime",
        "approved_at": "2026-05-15T00:00:00Z",
        "approval_hash": format!("approval-hash-pg-runtime-{suffix}")
    });
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
            "idempotency_key": format!("idem-pg-runtime-{suffix}")
        })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "submit response: {submit}");
    assert_eq!(submit["status"], "BLOCKED");

    let (client, connection) = tokio_postgres::connect(&database_url, tokio_postgres::NoTls)
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
