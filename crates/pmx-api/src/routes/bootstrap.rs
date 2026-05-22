use super::*;
use axum::routing::{get, post};

fn router_with_state(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/intents/normalize", post(flow::normalize))
        .route("/v1/snapshots/capture", post(flow::capture_snapshot))
        .route("/v1/decisions/evaluate", post(flow::decide))
        .route("/v1/plans/compile", post(flow::compile_plan))
        .route("/v1/submissions", post(flow::submit_plan))
        .route("/v1/submissions/:execution_id", get(read::get_submission))
        .route(
            "/v1/sign-only/lifecycle-events",
            post(flow::record_sign_only_lifecycle_event),
        )
        .route(
            "/v1/sign-only/standard-constructions",
            post(flow::record_standard_sign_only_construction),
        )
        .route(
            "/v1/sign-only/lifecycle-events/:execution_id",
            get(read::list_sign_only_lifecycle_events),
        )
        .route(
            "/v1/lifecycle/executions/:execution_id/events",
            get(read::list_execution_lifecycle_events),
        )
        .route(
            "/v1/lifecycle/orders/:order_id/events",
            get(read::list_order_lifecycle_events),
        )
        .route("/v1/runtime/workers", get(read::list_runtime_worker_status))
        .route(
            "/v1/admin/audit-events",
            get(admin::list_admin_audit_events),
        )
        .route("/v1/admin/kill-switch", post(admin::set_kill_switch))
        .route(
            "/v1/admin/cancel-order",
            post(admin::cancel_order_placeholder),
        )
        .route("/v1/admin/reconcile", post(admin::reconcile_placeholder))
        .route(
            "/v1/admin/reconcile-order-local",
            post(admin::reconcile_order_local),
        )
        .with_state(state)
}

pub fn try_app() -> Result<Router, String> {
    validate_auth_config_from_env()?;
    Ok(router_with_state(AppState::default()))
}

pub fn try_in_memory_app_with_store(store: InMemoryStore) -> Result<Router, String> {
    validate_auth_config_from_env()?;
    Ok(router_with_state(AppState::in_memory_with_store(store)))
}

pub fn app() -> Router {
    try_app().expect("PM_EXEC_SERVICE_TOKEN and PM_EXEC_ADMIN_TOKEN must be non-empty and distinct")
}

/// Build an HTTP API backed by a PostgreSQL store.
///
/// This helper is intended for integration tests and non-live smoke environments. It applies the
/// schema only when requested by the caller. The resulting API still blocks live submit; it only
/// proves the server-authoritative object graph and submit receipt path against PostgreSQL.
pub async fn try_postgres_app(
    database_url: impl Into<String>,
    apply_schema: bool,
) -> Result<Router, String> {
    validate_auth_config_from_env()?;
    let store = PostgresStore::connect(database_url.into())
        .await
        .map_err(|err| format!("postgres connect failed: {err}"))?;
    if apply_schema {
        store
            .apply_schema()
            .await
            .map_err(|err| format!("postgres schema apply failed: {err}"))?;
    }
    Ok(router_with_state(AppState::postgres(store)))
}
