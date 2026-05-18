use super::*;

async fn compile_blocked_plan(app: axum::Router) -> (String, String) {
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
            "approval": {
                "approval_id": "approval-v07-1",
                "approved_by": "operator-v07",
                "approved_at": "2026-05-14T00:00:00Z",
                "approval_hash": "approval-hash-v07-1"
            }
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

async fn verify_submit_and_sign_only(app: axum::Router, execution_id: &str, plan_hash: &str) {
    let (status, submit) = request_json(
        app.clone(),
        "POST",
        "/v1/submissions",
        Some("service-token-test-v07"),
        Some(json!({
            "execution_id": execution_id,
            "plan_hash": plan_hash,
            "idempotency_key": "idem-v07-1"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "submit response: {submit}");
    assert_eq!(submit["status"], "BLOCKED");

    let submission_uri = format!(
        "/v1/submissions/{}",
        submit["execution_id"].as_str().unwrap()
    );
    let (status, submission) = request_json(
        app.clone(),
        "GET",
        &submission_uri,
        Some("service-token-test-v07"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "submission response: {submission}");

    let (status, standard_sign_only) = request_json(
        app.clone(),
        "POST",
        "/v1/sign-only/standard-constructions",
        Some("service-token-test-v07"),
        Some(json!({
            "execution_id": execution_id,
            "account_id": "acct-http-e2e-1",
            "plan_hash": plan_hash,
            "no_remote_side_effect": true
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "standard sign-only response: {standard_sign_only}"
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
        Some("service-token-test-v07"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "sign-only list: {sign_only_records}"
    );
    assert_eq!(sign_only_records.as_array().unwrap().len(), 3);
    assert_eq!(sign_only_records[2]["state"], "SIGNED_DRY_RUN");

    let (status, invalid_sign_only) = request_json(
        app,
        "POST",
        "/v1/sign-only/lifecycle-events",
        Some("service-token-test-v07"),
        Some(json!({
            "execution_id": execution_id,
            "account_id": "acct-http-e2e-1",
            "state": "SIGNED_DRY_RUN",
            "event": "SIGNED_WITHOUT_POST",
            "signed_order_ref": "signed-order-ref-v23-replay",
            "no_remote_side_effect": false
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "unsafe sign-only lifecycle response: {invalid_sign_only}"
    );
}

async fn verify_non_live_admin_paths(app: axum::Router, execution_id: &str) {
    let (status, _) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/cancel-order",
        Some("service-token-test-v07"),
        Some(json!({"account_id": "acct-http-e2e-1", "order_id": "order-v07-1", "execution_id": execution_id, "reason": "service must not cancel"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, cancel) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/cancel-order",
        Some("admin-token-test-v07"),
        Some(json!({"account_id": "acct-http-e2e-1", "order_id": "order-v07-1", "execution_id": execution_id, "reason": "admin cancel smoke"})),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "cancel response: {cancel}");
    assert_eq!(cancel["state"], "RECONCILE_REQUIRED");

    let (status, reconcile) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/reconcile",
        Some("admin-token-test-v07"),
        Some(json!({"account_id": "acct-http-e2e-1", "execution_id": execution_id, "reason": "admin reconcile smoke"})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "reconcile response: {reconcile}"
    );
    assert_eq!(reconcile["checked_orders"], 0);

    let (status, reconcile_bad_pair) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/reconcile",
        Some("admin-token-test-v07"),
        Some(json!({
            "account_id": "acct-http-e2e-1",
            "execution_id": execution_id,
            "order_id": "order-v07-1",
            "reason": "admin reconcile bad pair"
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "reconcile bad pair response: {reconcile_bad_pair}"
    );

    let (status, reconcile_missing_order) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/reconcile",
        Some("admin-token-test-v07"),
        Some(json!({
            "account_id": "acct-http-e2e-1",
            "execution_id": execution_id,
            "order_id": "missing-order-v24-public",
            "remote_observation": "MISSING",
            "reason": "admin reconcile public local missing"
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "reconcile missing order response: {reconcile_missing_order}"
    );

    let (status, local_reconcile_missing) = request_json(
        app,
        "POST",
        "/v1/admin/reconcile-order-local",
        Some("admin-token-test-v07"),
        Some(json!({
            "account_id": "acct-http-e2e-1",
            "order_id": "missing-order-v24-1",
            "remote_observation": "MISSING",
            "reason": "admin local reconcile missing smoke"
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "local reconcile missing response: {local_reconcile_missing}"
    );
    assert!(local_reconcile_missing["correlation_id"].as_str().is_some());
}

async fn verify_public_queries(app: axum::Router, execution_id: &str) {
    let lifecycle_uri = format!("/v1/lifecycle/executions/{execution_id}/events");
    let (status, lifecycle_events) = request_json(
        app.clone(),
        "GET",
        &lifecycle_uri,
        Some("service-token-test-v07"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "lifecycle events: {lifecycle_events}"
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

    let (status, order_events) = request_json(
        app.clone(),
        "GET",
        "/v1/lifecycle/orders/order-v07-1/events",
        Some("service-token-test-v07"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "order events: {order_events}");
    assert!(order_events.as_array().unwrap().is_empty());

    let (status, runtime_workers) = request_json(
        app.clone(),
        "GET",
        "/v1/runtime/workers?account_id=acct-http-e2e-1&limit=20",
        Some("service-token-test-v07"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "runtime workers: {runtime_workers}");
    assert!(runtime_workers["heartbeats"].as_array().unwrap().is_empty());
    assert!(
        runtime_workers["observations"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let (status, _) = request_json(
        app.clone(),
        "GET",
        "/v1/admin/audit-events?limit=20",
        Some("service-token-test-v07"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, audit_events) = request_json(
        app,
        "GET",
        "/v1/admin/audit-events?limit=20",
        Some("admin-token-test-v07"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "audit events: {audit_events}");
    assert!(audit_events.as_array().unwrap().len() >= 2);
}

#[tokio::test]
async fn full_scaffold_path_compile_submit_cancel_and_reconcile() {
    unsafe {
        std::env::set_var("PM_EXEC_SERVICE_TOKEN", "service-token-test-v07");
        std::env::set_var("PM_EXEC_ADMIN_TOKEN", "admin-token-test-v07");
    }

    let app = pmx_api::app();
    let (execution_id, plan_hash) = compile_blocked_plan(app.clone()).await;
    verify_submit_and_sign_only(app.clone(), &execution_id, &plan_hash).await;
    verify_non_live_admin_paths(app.clone(), &execution_id).await;
    verify_public_queries(app, &execution_id).await;
}
