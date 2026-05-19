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

    let app = pmx_api::app();
    let (execution_id, plan_hash) = compile_plan::compile_blocked_plan(app.clone()).await;
    submit_sign_only::verify_submit_and_sign_only(app.clone(), &execution_id, &plan_hash).await;
    admin_paths::verify_non_live_admin_paths(app.clone(), &execution_id).await;
    public_queries::verify_public_queries(app, &execution_id).await;
}
