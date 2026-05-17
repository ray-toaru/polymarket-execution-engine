use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};
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
        builder = builder.header("Authorization", bearer(token));
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
        "client_intent_id": "intent-http-pg-e2e-1",
        "account_id": "acct-http-pg-e2e-1",
        "market": {"condition_id": "cond-http-pg-e2e-1", "slug": null, "is_sports": false},
        "token_id": "token-http-pg-e2e-1",
        "side": "BUY",
        "quantity": {"max_notional": "10", "max_shares": null},
        "limit_price": "0.55",
        "time_in_force": "GTC",
        "collateral_profile_id": null
    })
}

fn unique_suffix(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    format!("{prefix}-{nanos}")
}

async fn seed_allow_runtime(
    database_url: &str,
    account_id: &str,
    condition_id: &str,
    suffix: &str,
) {
    let (client, connection) = tokio_postgres::connect(database_url, tokio_postgres::NoTls)
        .await
        .expect("connect for runtime seed");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
        .execute(
            "INSERT INTO runtime_accounts (account_id, status, kill_switch_enabled) \
             VALUES ($1, 'ACTIVE', false) \
             ON CONFLICT (account_id) DO UPDATE SET status = EXCLUDED.status, kill_switch_enabled = EXCLUDED.kill_switch_enabled, updated_at = now()",
            &[&account_id],
        )
        .await
        .expect("seed runtime account");
    client
        .execute(
            "INSERT INTO runtime_markets (condition_id, status, is_sports) \
             VALUES ($1, 'ACTIVE', false) \
             ON CONFLICT (condition_id) DO UPDATE SET status = EXCLUDED.status, is_sports = EXCLUDED.is_sports, updated_at = now()",
            &[&condition_id],
        )
        .await
        .expect("seed runtime market");
    let profile_id = format!("default-profile-{suffix}");
    client
        .execute(
            "INSERT INTO collateral_profiles (profile_id, status, quote_asset_symbol, quote_asset_address, allowance_target, decimals, profile_version) \
             VALUES ($1, 'DEFAULT_RESOLVED', 'pUSD', '0x0000000000000000000000000000000000000001', '0x0000000000000000000000000000000000000002', 6, 'test') \
             ON CONFLICT (profile_id) DO UPDATE SET status = EXCLUDED.status",
            &[&profile_id],
        )
        .await
        .expect("seed collateral profile");
    for capability in ["heartbeat", "reconcile", "resource-refresh"] {
        let worker_id = format!("worker-{suffix}-{capability}");
        let capability_value = capability.to_string();
        client
            .execute(
                "INSERT INTO worker_health (worker_id, role, capability, status, last_heartbeat_at) \
                 VALUES ($1, 'test', $2, 'HEALTHY', now()) \
                 ON CONFLICT (worker_id) DO UPDATE SET status = EXCLUDED.status, last_heartbeat_at = now(), updated_at = now()",
                &[&worker_id, &capability_value],
            )
            .await
            .expect("seed worker health");
    }
}

async fn seed_runtime_worker_observation(
    database_url: &str,
    account_id: &str,
    capability: &str,
    status: &str,
    should_fail_closed: bool,
    reason: &str,
) {
    let (client, connection) = tokio_postgres::connect(database_url, tokio_postgres::NoTls)
        .await
        .expect("connect for worker observation seed");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
        .execute(
            "INSERT INTO runtime_worker_observations \
             (account_id, capability, worker_kind, status, should_fail_closed, reason) \
             VALUES ($1, $2, 'http-pg-test', $3, $4, $5)",
            &[
                &account_id,
                &capability,
                &status,
                &should_fail_closed,
                &reason,
            ],
        )
        .await
        .expect("seed worker observation");
}

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
        "plan_hash": plan_hash,
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

    for record in [
        json!({
            "execution_id": execution_id.clone(),
            "account_id": format!("acct-http-pg-e2e-{suffix}"),
            "state": "RESERVATION_PREPARED",
            "event": "PREPARE_RESERVATION",
            "signed_order_ref": null,
            "no_remote_side_effect": true
        }),
        json!({
            "execution_id": execution_id.clone(),
            "account_id": format!("acct-http-pg-e2e-{suffix}"),
            "state": "SIGNING_REQUESTED",
            "event": "REQUEST_SIGNING",
            "signed_order_ref": null,
            "no_remote_side_effect": true
        }),
        json!({
            "execution_id": execution_id.clone(),
            "account_id": format!("acct-http-pg-e2e-{suffix}"),
            "state": "SIGNED_DRY_RUN",
            "event": "SIGNED_WITHOUT_POST",
            "signed_order_ref": format!("signed-order-ref-pg-e2e-{suffix}"),
            "no_remote_side_effect": true
        }),
    ] {
        let (status, recorded) = request_json(
            app.clone(),
            "POST",
            "/v1/sign-only/lifecycle-events",
            Some("service-token-pg-e2e"),
            Some(record),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::ACCEPTED,
            "sign-only PG lifecycle response: {recorded}"
        );
    }
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

fn sample_intent_variant(suffix: &str) -> Value {
    let mut value = sample_intent();
    value["client_intent_id"] = Value::String(format!("intent-http-pg-e2e-{suffix}"));
    value["account_id"] = Value::String(format!("acct-http-pg-e2e-{suffix}"));
    value["market"]["condition_id"] = Value::String(format!("cond-http-pg-e2e-{suffix}"));
    value["token_id"] = Value::String(format!("token-http-pg-e2e-{suffix}"));
    value
}

#[tokio::test]
async fn http_postgres_rejects_cross_object_graph_and_bad_plan_hash() {
    let Ok(database_url) = std::env::var("PMX_TEST_DATABASE_URL") else {
        eprintln!("PMX_TEST_DATABASE_URL not set; skipping HTTP PostgreSQL negative E2E smoke");
        return;
    };
    unsafe {
        std::env::set_var("PM_EXEC_SERVICE_TOKEN", "service-token-pg-negative");
        std::env::set_var("PM_EXEC_ADMIN_TOKEN", "admin-token-pg-negative");
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

    let approval = json!({
        "approval_id": "approval-pg-negative-1",
        "approved_by": "operator-pg-negative",
        "approved_at": "2026-05-15T00:00:00Z",
        "approval_hash": "approval-hash-pg-negative-1"
    });
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
            "idempotency_key": "idem-pg-negative-1"
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "bad submit response: {bad_submit}"
    );
}

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

#[tokio::test]
async fn http_postgres_admin_routes_record_audit_events() {
    let Ok(database_url) = std::env::var("PMX_TEST_DATABASE_URL") else {
        eprintln!("PMX_TEST_DATABASE_URL not set; skipping HTTP PostgreSQL admin audit E2E smoke");
        return;
    };
    unsafe {
        std::env::set_var("PM_EXEC_SERVICE_TOKEN", "service-token-pg-audit");
        std::env::set_var("PM_EXEC_ADMIN_TOKEN", "admin-token-pg-audit");
    }

    let app = pmx_api::try_postgres_app(database_url.clone(), true)
        .await
        .expect("postgres-backed app");

    let (status, receipt) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/kill-switch",
        Some("admin-token-pg-audit"),
        Some(json!({"enabled": true, "reason": "audit e2e"})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "kill-switch response: {receipt}"
    );

    let (status, cancel) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/cancel-order",
        Some("admin-token-pg-audit"),
        Some(json!({"account_id": "acct-audit", "order_id": "order-audit", "reason": "audit e2e"})),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "cancel response: {cancel}");

    let (status, rejected) = request_json(
        app.clone(),
        "POST",
        "/v1/admin/cancel-order",
        Some("admin-token-pg-audit"),
        Some(json!({"account_id": "acct-audit", "order_id": "order-audit-rejected", "reason": ""})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "rejected cancel response: {rejected}"
    );

    let (client, connection) = tokio_postgres::connect(&database_url, tokio_postgres::NoTls)
        .await
        .expect("connect for audit count");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let row = client
        .query_one(
            "SELECT COUNT(*)::bigint FROM admin_audit_events WHERE principal_subject = 'admin-token' AND operation IN ('KillSwitch', 'CancelOrder')",
            &[],
        )
        .await
        .expect("count audit events");
    let count: i64 = row.get(0);
    assert!(
        count >= 2,
        "expected at least two admin audit events, got {count}"
    );
    let rejected_row = client
        .query_one(
            "SELECT COUNT(*)::bigint FROM admin_audit_events WHERE principal_subject = 'admin-token' AND operation = 'CancelOrder' AND result LIKE 'REJECTED%'",
            &[],
        )
        .await
        .expect("count rejected audit events");
    let rejected_count: i64 = rejected_row.get(0);
    assert!(rejected_count >= 1, "expected rejected cancel audit event");

    let (status, _) = request_json(
        app.clone(),
        "GET",
        "/v1/admin/audit-events?limit=20",
        Some("service-token-pg-audit"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, audit_events) = request_json(
        app,
        "GET",
        "/v1/admin/audit-events?limit=20",
        Some("admin-token-pg-audit"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "audit query response: {audit_events}"
    );
    assert!(audit_events.as_array().unwrap().len() >= 2);
}
