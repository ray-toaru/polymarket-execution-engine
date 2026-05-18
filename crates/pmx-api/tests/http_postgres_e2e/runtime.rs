use super::*;

#[path = "runtime/ready_plan.rs"]
mod ready_plan;

#[path = "runtime/runtime_state.rs"]
mod runtime_state;

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

    let (normalized, ready_snapshot, ready_decision) =
        runtime_state::verify_ready_and_degraded_runtime(
            app.clone(),
            &database_url,
            &account_id,
            intent,
        )
        .await;
    ready_plan::verify_ready_plan_and_blocked_submit(
        app,
        &database_url,
        &suffix,
        normalized,
        ready_snapshot,
        ready_decision,
    )
    .await;
}
