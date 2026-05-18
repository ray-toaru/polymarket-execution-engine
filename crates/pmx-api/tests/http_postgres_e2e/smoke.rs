use super::*;

#[tokio::test]
async fn http_postgres_backed_e2e_smoke() {
    let Ok(database_url) = std::env::var("PMX_TEST_DATABASE_URL") else {
        eprintln!("PMX_TEST_DATABASE_URL not set; skipping HTTP PostgreSQL E2E smoke");
        return;
    };
    unsafe {
        std::env::set_var("PM_EXEC_SERVICE_TOKEN", "service-token-pg-e2e");
        std::env::set_var("PM_EXEC_ADMIN_TOKEN", "admin-token-pg-e2e");
    }

    let suffix = unique_suffix("smoke");
    let app = pmx_api::try_postgres_app(database_url, true)
        .await
        .expect("postgres-backed app");
    let intent = sample_intent_variant(&suffix);

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
        app.clone(),
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

    let (status, standard_sign_only) = request_json(
        app.clone(),
        "POST",
        "/v1/sign-only/standard-constructions",
        Some("service-token-pg-e2e"),
        Some(json!({
            "execution_id": execution_id.clone(),
            "account_id": format!("acct-http-pg-e2e-{suffix}"),
            "plan_hash": plan_hash,
            "no_remote_side_effect": true
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "standard sign-only PG response: {standard_sign_only}"
    );
    assert_eq!(standard_sign_only["no_remote_side_effect"], true);
    assert!(
        standard_sign_only["signed_order_ref"]
            .as_str()
            .unwrap()
            .starts_with(&format!("sign-only:{execution_id}:"))
    );
    assert_eq!(
        standard_sign_only["signed_order_digest"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    let sign_only_uri = format!("/v1/sign-only/lifecycle-events/{execution_id}");
    let (status, sign_only_records) = request_json(
        app.clone(),
        "GET",
        &sign_only_uri,
        Some("service-token-pg-e2e"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "sign-only PG list: {sign_only_records}"
    );
    assert_eq!(sign_only_records.as_array().unwrap().len(), 3);

    let (status, cancel) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/cancel-order",
        Some("admin-token-pg-e2e"),
        Some(json!({
            "account_id": format!("acct-http-pg-e2e-{suffix}"),
            "order_id": format!("order-pg-e2e-{suffix}"),
            "execution_id": execution_id.clone(),
            "reason": "pg cancel lifecycle smoke"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "cancel response: {cancel}");

    let (status, reconcile) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/reconcile",
        Some("admin-token-pg-e2e"),
        Some(json!({
            "account_id": format!("acct-http-pg-e2e-{suffix}"),
            "execution_id": execution_id.clone(),
            "reason": "pg reconcile lifecycle smoke"
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "reconcile response: {reconcile}"
    );

    let lifecycle_uri = format!("/v1/lifecycle/executions/{execution_id}/events");
    let (status, lifecycle_events) = request_json(
        app.clone(),
        "GET",
        &lifecycle_uri,
        Some("service-token-pg-e2e"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "PG lifecycle events: {lifecycle_events}"
    );
    let event_types: Vec<_> = lifecycle_events
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["event_type"].as_str().unwrap().to_string())
        .collect();
    assert!(event_types.contains(&"CANCEL_REQUESTED_NON_LIVE".to_string()));
    assert!(event_types.contains(&"RECONCILE_REQUESTED_NON_LIVE".to_string()));
    for event in lifecycle_events.as_array().unwrap() {
        if matches!(
            event["event_type"].as_str().unwrap(),
            "CANCEL_REQUESTED_NON_LIVE" | "RECONCILE_REQUESTED_NON_LIVE"
        ) {
            assert_eq!(event["payload"]["schema_version"], 1);
            assert!(event["payload"]["correlation_id"].as_str().is_some());
            assert_eq!(event["payload"]["body"]["no_remote_side_effect"], true);
            assert!(
                event["payload"]["redacted_fields"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("signed_payload"))
            );
        }
    }

    let order_events_uri = format!("/v1/lifecycle/orders/order-http-pg-e2e-{suffix}/events");
    let (status, order_events) = request_json(
        app.clone(),
        "GET",
        &order_events_uri,
        Some("service-token-pg-e2e"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "PG order events: {order_events}");
    assert!(order_events.as_array().unwrap().is_empty());

    let (status, audit_events) = request_json(
        app,
        "GET",
        "/v1/admin/audit-events?limit=20",
        Some("admin-token-pg-e2e"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "PG audit query: {audit_events}");
    assert!(audit_events.as_array().unwrap().len() >= 2);
}
