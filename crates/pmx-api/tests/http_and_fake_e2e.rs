use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use std::sync::OnceLock;
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

fn test_hash(label: &str) -> String {
    pmx_core::canonical_json_sha256(&format!("api-test-{label}"))
        .expect("test hash")
        .0
}

fn approval_json(approval_id: &str, snapshot: &Value, decision: &Value) -> Value {
    json!({
        "approval_id": approval_id,
        "approved_by": "operator-http-e2e",
        "approved_at": "2026-05-14T00:00:00Z",
        "expires_at": "2030-01-01T00:00:00Z",
        "approval_scope": "SHADOW",
        "approval_hash": test_hash(approval_id),
        "bound_artifact_sha256": test_hash("artifact"),
        "bound_evidence_manifest_sha256": test_hash("evidence-manifest"),
        "bound_snapshot_hash": snapshot["snapshot_hash"],
        "bound_decision_hash": decision["decision_hash"],
        "bound_plan_hash": null,
        "operator_identity_ref": "local-http-e2e-operator"
    })
}

async fn seed_in_memory_cancelable_order(
    store: &pmx_store::InMemoryStore,
    account_id: &str,
    order_id: &str,
    execution_id: &str,
) {
    use pmx_core::OrderLifecycleState;
    use pmx_store::{OrderLifecycleRecord, OrderLifecycleStore};

    store
        .upsert_order_lifecycle(&OrderLifecycleRecord {
            order_id: order_id.to_owned(),
            execution_id: execution_id.to_owned(),
            account_id: account_id.to_owned(),
            condition_id: "cond-http-e2e-1".into(),
            token_id: "token-http-e2e-1".into(),
            side: "BUY".into(),
            lifecycle_state: OrderLifecycleState::Posted,
            remote_order_id: Some(format!("remote-{order_id}")),
            remote_state: Some("OPEN".into()),
            created_at: None,
            updated_at: None,
        })
        .await
        .expect("seed in-memory cancelable order");
}

#[path = "http_and_fake_e2e/smoke.rs"]
mod smoke;

#[path = "http_and_fake_e2e/scaffold.rs"]
mod scaffold;

#[path = "http_and_fake_e2e/negative.rs"]
mod negative;
