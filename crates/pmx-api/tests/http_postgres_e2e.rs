use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, MutexGuard};
use tower::ServiceExt;

async fn env_lock() -> MutexGuard<'static, ()> {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    ENV_LOCK.get_or_init(|| Mutex::new(())).lock().await
}

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

async fn request_json_with_headers(
    app: axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
    headers: &[(&str, &str)],
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("Authorization", bearer(token));
    }
    for (name, value) in headers {
        builder = builder.header(*name, *value);
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

fn test_hash(label: &str) -> String {
    pmx_core::canonical_json_sha256(&format!("api-pg-test-{label}"))
        .expect("test hash")
        .0
}

fn zero_hash() -> String {
    "0000000000000000000000000000000000000000000000000000000000000000".into()
}

fn approval_json(approval_id: &str, snapshot: &Value, decision: &Value) -> Value {
    let mut value = json!({
        "approval_id": approval_id,
        "approved_by": "operator-pg-e2e",
        "approved_at": "2026-05-15T00:00:00Z",
        "expires_at": "2030-01-01T00:00:00Z",
        "approval_scope": "SHADOW",
        "approval_hash": zero_hash(),
        "bound_artifact_sha256": test_hash("artifact"),
        "bound_evidence_manifest_sha256": test_hash("evidence-manifest"),
        "bound_snapshot_hash": snapshot["snapshot_hash"],
        "bound_decision_hash": decision["decision_hash"],
        "bound_plan_hash": null,
        "operator_identity_ref": "local-pg-e2e-operator"
    });
    let approval: pmx_core::ApprovalReceipt =
        serde_json::from_value(value.clone()).expect("approval json");
    value["approval_hash"] = json!(
        pmx_service::approval_receipt_hash(&approval)
            .expect("approval hash")
            .0
    );
    value
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

async fn seed_cancelable_order(
    database_url: &str,
    account_id: &str,
    order_id: &str,
    execution_id: &str,
    condition_id: &str,
    token_id: &str,
) {
    let (client, connection) = tokio_postgres::connect(database_url, tokio_postgres::NoTls)
        .await
        .expect("connect for cancelable order seed");
    tokio::spawn(async move {
        let _ = connection.await;
    });

    let suffix = unique_suffix("cancel-seed");
    let normalized_intent_id = format!("norm-{suffix}");
    let snapshot_id = format!("snap-{suffix}");
    let decision_id = format!("decision-{suffix}");
    let intent_hash = format!("intent-hash-{suffix}");
    let snapshot_hash = format!("snapshot-hash-{suffix}");
    let decision_hash = format!("decision-hash-{suffix}");
    let plan_hash = format!("plan-hash-{suffix}");

    client
        .execute(
            "INSERT INTO normalized_intents (normalized_intent_id, intent_hash, account_id, payload) \
             VALUES ($1, $2, $3, '{}'::jsonb)",
            &[&normalized_intent_id, &intent_hash, &account_id],
        )
        .await
        .expect("seed cancel normalized intent");
    client
        .execute(
            "INSERT INTO feasibility_snapshots (snapshot_id, snapshot_hash, normalized_intent_id, payload, captured_at) \
             VALUES ($1, $2, $3, '{}'::jsonb, now())",
            &[&snapshot_id, &snapshot_hash, &normalized_intent_id],
        )
        .await
        .expect("seed cancel snapshot");
    client
        .execute(
            "INSERT INTO constraint_decisions (decision_id, decision_hash, snapshot_id, status, reasons, payload) \
             VALUES ($1, $2, $3, 'ALLOW', '[]'::jsonb, '{}'::jsonb)",
            &[&decision_id, &decision_hash, &snapshot_id],
        )
        .await
        .expect("seed cancel decision");
    client
        .execute(
            "INSERT INTO execution_plans (execution_id, account_id, normalized_intent_id, snapshot_id, decision_id, plan_hash, status, summary_json) \
             VALUES ($1, $2, $3, $4, $5, $6, 'READY', '{}'::jsonb) \
             ON CONFLICT (execution_id) DO UPDATE SET \
               account_id = EXCLUDED.account_id, \
               normalized_intent_id = EXCLUDED.normalized_intent_id, \
               snapshot_id = EXCLUDED.snapshot_id, \
               decision_id = EXCLUDED.decision_id, \
               plan_hash = EXCLUDED.plan_hash, \
               status = EXCLUDED.status, \
               summary_json = EXCLUDED.summary_json, \
               updated_at = now()",
            &[
                &execution_id,
                &account_id,
                &normalized_intent_id,
                &snapshot_id,
                &decision_id,
                &plan_hash,
            ],
        )
        .await
        .expect("seed cancel execution plan");
    let remote_order_id = format!("remote-{order_id}");
    client
        .execute(
            "INSERT INTO orders \
             (order_id, execution_id, account_id, condition_id, token_id, side, lifecycle_state, remote_order_id, remote_state, updated_at) \
             VALUES ($1, $2, $3, $4, $5, 'BUY', 'POSTED', $6, 'OPEN', now()) \
             ON CONFLICT (order_id) DO UPDATE SET \
               execution_id = EXCLUDED.execution_id, \
               account_id = EXCLUDED.account_id, \
               condition_id = EXCLUDED.condition_id, \
               token_id = EXCLUDED.token_id, \
               side = EXCLUDED.side, \
               lifecycle_state = EXCLUDED.lifecycle_state, \
               remote_order_id = EXCLUDED.remote_order_id, \
               remote_state = EXCLUDED.remote_state, \
               updated_at = now()",
            &[
                &order_id,
                &execution_id,
                &account_id,
                &condition_id,
                &token_id,
                &remote_order_id,
            ],
        )
        .await
        .expect("seed cancelable order");
}

#[path = "http_postgres_e2e/admin_audit.rs"]
mod admin_audit;

#[path = "http_postgres_e2e/negative.rs"]
mod negative;

fn sample_intent_variant(suffix: &str) -> Value {
    let mut value = sample_intent();
    value["client_intent_id"] = Value::String(format!("intent-http-pg-e2e-{suffix}"));
    value["account_id"] = Value::String(format!("acct-http-pg-e2e-{suffix}"));
    value["market"]["condition_id"] = Value::String(format!("cond-http-pg-e2e-{suffix}"));
    value["token_id"] = Value::String(format!("token-http-pg-e2e-{suffix}"));
    value
}

#[path = "http_postgres_e2e/runtime.rs"]
mod runtime;

#[path = "http_postgres_e2e/smoke.rs"]
mod smoke;
