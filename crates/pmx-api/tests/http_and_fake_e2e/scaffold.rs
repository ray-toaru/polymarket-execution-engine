use super::*;

#[path = "scaffold/admin_paths.rs"]
mod admin_paths;

#[path = "scaffold/compile_plan.rs"]
mod compile_plan;

#[path = "scaffold/public_queries.rs"]
mod public_queries;

#[path = "scaffold/submit_sign_only.rs"]
mod submit_sign_only;

#[tokio::test]
async fn full_scaffold_path_compile_submit_cancel_and_reconcile() {
    let _guard = env_lock().await;
    unsafe {
        std::env::set_var("PM_EXEC_SERVICE_TOKEN", "service-token-test-v07");
        std::env::set_var("PM_EXEC_ADMIN_TOKEN", "admin-token-test-v07");
    }

    let store = pmx_store::InMemoryStore::default();
    let app = pmx_api::try_in_memory_app_with_store(store.clone()).expect("in-memory app");
    let (execution_id, plan_hash) = compile_plan::compile_blocked_plan(app.clone()).await;
    submit_sign_only::verify_submit_and_sign_only(app.clone(), &execution_id, &plan_hash).await;
    seed_in_memory_cancelable_order(&store, "acct-http-e2e-1", "order-v07-1", &execution_id).await;
    admin_paths::verify_non_live_admin_paths(app.clone(), &execution_id).await;
    public_queries::verify_public_queries(app, &execution_id).await;
}
