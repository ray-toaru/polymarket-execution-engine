use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

async fn request_json(
    app: axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", bearer(token));
    }
    let body = match body {
        Some(value) => Body::from(value.to_string()),
        None => Body::empty(),
    };
    let response = app
        .oneshot(builder.body(body).expect("request body"))
        .await
        .expect("router response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("json response")
    };
    (status, value)
}

fn sample_intent() -> Value {
    json!({
        "client_intent_id": "intent-http-e2e-1",
        "account_id": "acct-http-e2e-1",
        "market": {"condition_id": "cond-http-e2e-1", "slug": null, "is_sports": false},
        "token_id": "token-http-e2e-1",
        "side": "BUY",
        "quantity": {"max_notional": "10", "max_shares": null},
        "limit_price": "0.55",
        "time_in_force": "GTC",
        "collateral_profile_id": null
    })
}

#[tokio::test]
async fn http_auth_and_fake_e2e_smoke() {
    unsafe {
        std::env::set_var("PM_EXEC_SERVICE_TOKEN", "service-token-test");
        std::env::set_var("PM_EXEC_ADMIN_TOKEN", "admin-token-test");
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
        Some(json!({"enabled": true, "reason": "negative auth test"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, kill_switch) = request_json(
        app,
        "POST",
        "/v1/admin/kill-switch",
        Some("admin-token-test"),
        Some(json!({"enabled": true, "reason": "admin auth test"})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "kill-switch response: {kill_switch}"
    );
    assert_eq!(kill_switch["enabled"], true);
}

#[tokio::test]
async fn full_scaffold_path_compile_submit_cancel_and_reconcile() {
    unsafe {
        std::env::set_var("PM_EXEC_SERVICE_TOKEN", "service-token-test-v07");
        std::env::set_var("PM_EXEC_ADMIN_TOKEN", "admin-token-test-v07");
    }

    let app = pmx_api::app();

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

    let plan_normalized_id = normalized["normalized_intent_id"].clone();
    let plan_snapshot_id = snapshot["snapshot_id"].clone();
    let approval = json!({
        "approval_id": "approval-v07-1",
        "approved_by": "operator-v07",
        "approved_at": "2026-05-14T00:00:00Z",
        "approval_hash": "approval-hash-v07-1"
    });
    let (status, plan) = request_json(
        app.clone(),
        "POST",
        "/v1/plans/compile",
        Some("service-token-test-v07"),
        Some(json!({
            "normalized_intent_id": plan_normalized_id,
            "snapshot_id": plan_snapshot_id,
            "decision_id": decision["decision_id"],
            "approval": approval
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "plan response: {plan}");
    assert_eq!(plan["status"], "BLOCKED");

    let execution_id = plan["execution_id"]
        .as_str()
        .expect("execution_id")
        .to_string();
    let plan_hash = plan["plan_hash"].as_str().expect("plan_hash").to_string();
    let (status, submit) = request_json(
        app.clone(),
        "POST",
        "/v1/submissions",
        Some("service-token-test-v07"),
        Some(json!({
            "execution_id": execution_id.clone(),
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

    for record in [
        json!({
            "execution_id": execution_id.clone(),
            "account_id": "acct-http-e2e-1",
            "state": "RESERVATION_PREPARED",
            "event": "PREPARE_RESERVATION",
            "signed_order_ref": null,
            "no_remote_side_effect": true
        }),
        json!({
            "execution_id": execution_id.clone(),
            "account_id": "acct-http-e2e-1",
            "state": "SIGNING_REQUESTED",
            "event": "REQUEST_SIGNING",
            "signed_order_ref": null,
            "no_remote_side_effect": true
        }),
        json!({
            "execution_id": execution_id.clone(),
            "account_id": "acct-http-e2e-1",
            "state": "SIGNED_DRY_RUN",
            "event": "SIGNED_WITHOUT_POST",
            "signed_order_ref": "signed-order-ref-v23-fake",
            "no_remote_side_effect": true
        }),
    ] {
        let (status, recorded) = request_json(
            app.clone(),
            "POST",
            "/v1/sign-only/lifecycle-events",
            Some("service-token-test-v07"),
            Some(record),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::ACCEPTED,
            "sign-only lifecycle response: {recorded}"
        );
    }

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
        app.clone(),
        "POST",
        "/v1/sign-only/lifecycle-events",
        Some("service-token-test-v07"),
        Some(json!({
            "execution_id": execution_id.clone(),
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

    let (status, _) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/cancel-order",
        Some("service-token-test-v07"),
        Some(json!({"account_id": "acct-http-e2e-1", "order_id": "order-v07-1", "execution_id": execution_id.clone(), "reason": "service must not cancel"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, cancel) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/cancel-order",
        Some("admin-token-test-v07"),
        Some(json!({"account_id": "acct-http-e2e-1", "order_id": "order-v07-1", "execution_id": execution_id.clone(), "reason": "admin cancel smoke"})),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "cancel response: {cancel}");
    assert_eq!(cancel["state"], "RECONCILE_REQUIRED");

    let (status, reconcile) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/reconcile",
        Some("admin-token-test-v07"),
        Some(json!({"account_id": "acct-http-e2e-1", "execution_id": execution_id.clone(), "reason": "admin reconcile smoke"})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "reconcile response: {reconcile}"
    );
    assert_eq!(reconcile["checked_orders"], 0);

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

#[test]
fn equal_service_and_admin_tokens_fail_closed_at_app_construction() {
    unsafe {
        std::env::set_var("PM_EXEC_SERVICE_TOKEN", "same-token-test");
        std::env::set_var("PM_EXEC_ADMIN_TOKEN", "same-token-test");
    }
    let err = pmx_api::try_app().expect_err("equal tokens must fail closed");
    assert!(err.contains("distinct"));
}

#[tokio::test]
async fn mismatched_object_graph_is_rejected() {
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
