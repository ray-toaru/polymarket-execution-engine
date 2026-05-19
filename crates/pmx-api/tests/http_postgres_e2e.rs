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
