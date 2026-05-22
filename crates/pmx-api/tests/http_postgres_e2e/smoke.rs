use super::*;

#[path = "smoke/admin_lifecycle.rs"]
mod admin_lifecycle;

#[path = "smoke/compile_submit.rs"]
mod compile_submit;

#[path = "smoke/public_queries.rs"]
mod public_queries;

#[path = "smoke/sign_only.rs"]
mod sign_only;

#[tokio::test]
async fn http_postgres_backed_e2e_smoke() {
    let _guard = env_lock().await;
    let Ok(database_url) = std::env::var("PMX_TEST_DATABASE_URL") else {
        eprintln!("PMX_TEST_DATABASE_URL not set; skipping HTTP PostgreSQL E2E smoke");
        return;
    };
    unsafe {
        std::env::set_var("PM_EXEC_SERVICE_TOKEN", "service-token-pg-e2e");
        std::env::set_var("PM_EXEC_ADMIN_TOKEN", "admin-token-pg-e2e");
    }

    let suffix = unique_suffix("smoke");
    let app = pmx_api::try_postgres_app(database_url.clone(), true)
        .await
        .expect("postgres-backed app");
    let intent = sample_intent_variant(&suffix);

    let (execution_id, plan_hash) =
        compile_submit::compile_and_submit_blocked_plan(app.clone(), intent, &suffix).await;
    sign_only::verify_sign_only_flow(app.clone(), &execution_id, &plan_hash, &suffix).await;
    admin_lifecycle::verify_admin_cancel_and_reconcile(
        app.clone(),
        &database_url,
        &execution_id,
        &suffix,
    )
    .await;
    public_queries::verify_public_queries(app, &execution_id, &suffix).await;
}
