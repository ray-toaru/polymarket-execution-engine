use crate::backend::{AppState, CONTRACT_VERSION};
use crate::support::{ApiResult, require, validate_auth_config_from_env};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use pmx_authz::Operation;
use pmx_store::{InMemoryStore, PostgresStore};

mod admin;
mod flow;
mod read;

#[path = "routes/bootstrap.rs"]
mod bootstrap;

#[path = "routes/health.rs"]
mod health;

use health::health;

pub fn try_app() -> Result<Router, String> {
    bootstrap::try_app()
}

pub fn app() -> Router {
    bootstrap::app()
}

pub fn try_in_memory_app_with_store(store: InMemoryStore) -> Result<Router, String> {
    bootstrap::try_in_memory_app_with_store(store)
}

pub async fn try_postgres_app(
    database_url: impl Into<String>,
    apply_schema: bool,
) -> Result<Router, String> {
    bootstrap::try_postgres_app(database_url, apply_schema).await
}
