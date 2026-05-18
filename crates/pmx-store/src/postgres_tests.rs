use super::*;
use crate::*;
use std::time::{SystemTime, UNIX_EPOCH};

async fn test_store() -> Option<PostgresStore> {
    let Ok(url) = std::env::var("PMX_TEST_DATABASE_URL") else {
        eprintln!("PMX_TEST_DATABASE_URL not set; skipping PostgreSQL repository test");
        return None;
    };
    let store = PostgresStore::connect(url).await.ok()?;
    store.apply_schema().await.expect("apply PostgreSQL schema");
    Some(store)
}

fn unique(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    format!("{prefix}-{nanos}")
}

async fn seed_execution_plan(store: &PostgresStore, account_id: &str, execution_id: &str) {
    let client = store.client().await.expect("test postgres client");
    let norm = unique("norm");
    let snap = unique("snap");
    let dec = unique("decision");
    let plan_hash = unique("plan-hash");
    client
        .execute(
            "INSERT INTO normalized_intents (normalized_intent_id, intent_hash, account_id, payload) \
             VALUES ($1, $2, $3, '{}'::jsonb)",
            &[&norm, &unique("intent-hash"), &account_id],
        )
        .await
        .expect("seed normalized intent");
    client
        .execute(
            "INSERT INTO feasibility_snapshots (snapshot_id, snapshot_hash, normalized_intent_id, payload, captured_at) \
             VALUES ($1, $2, $3, '{}'::jsonb, now())",
            &[&snap, &unique("snapshot-hash"), &norm],
        )
        .await
        .expect("seed snapshot");
    client
        .execute(
            "INSERT INTO constraint_decisions (decision_id, decision_hash, snapshot_id, status, reasons, payload) \
             VALUES ($1, $2, $3, 'ALLOW', '[]'::jsonb, '{}'::jsonb)",
            &[&dec, &unique("decision-hash"), &snap],
        )
        .await
        .expect("seed decision");
    client
        .execute(
            "INSERT INTO execution_plans (execution_id, account_id, normalized_intent_id, snapshot_id, decision_id, plan_hash, status, summary_json) \
             VALUES ($1, $2, $3, $4, $5, $6, 'READY', '{}'::jsonb)",
            &[&execution_id, &account_id, &norm, &snap, &dec, &plan_hash],
        )
        .await
        .expect("seed execution plan");
}

#[path = "postgres_tests/admin_audit.rs"]
mod admin_audit;
#[path = "postgres_tests/execution_lifecycle.rs"]
mod execution_lifecycle;
#[path = "postgres_tests/idempotency.rs"]
mod idempotency;
#[path = "postgres_tests/order_lifecycle.rs"]
mod order_lifecycle;
#[path = "postgres_tests/receipt_reservation.rs"]
mod receipt_reservation;
#[path = "postgres_tests/runtime_state.rs"]
mod runtime_state;
#[path = "postgres_tests/runtime_worker_health.rs"]
mod runtime_worker_health;
#[path = "postgres_tests/schema.rs"]
mod schema;
#[path = "postgres_tests/sign_only.rs"]
mod sign_only;
