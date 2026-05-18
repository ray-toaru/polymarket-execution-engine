use chrono::{Duration, Utc};
use pmx_core::{CollateralProfileStatus, GeoblockStatus, WorkerStatus};
use tokio_postgres::Client;

use crate::StoreError;

pub(crate) fn map_db_error(err: tokio_postgres::Error) -> StoreError {
    if let Some(db_error) = err.as_db_error() {
        if db_error.code() == &tokio_postgres::error::SqlState::UNIQUE_VIOLATION {
            return StoreError::Conflict(db_error.message().to_string());
        }
        if db_error.code() == &tokio_postgres::error::SqlState::FOREIGN_KEY_VIOLATION {
            return StoreError::NotFound(db_error.message().to_string());
        }
        if db_error.code() == &tokio_postgres::error::SqlState::CHECK_VIOLATION {
            return StoreError::Conflict(db_error.message().to_string());
        }
        if db_error.code() == &tokio_postgres::error::SqlState::T_R_SERIALIZATION_FAILURE {
            return StoreError::SerializationFailure;
        }
    }
    StoreError::DatabaseUnavailable(err.to_string())
}

pub(crate) async fn load_json_payload<T: serde::de::DeserializeOwned>(
    client: &Client,
    table: &str,
    id_column: &str,
    id_value: &str,
    payload_column: &str,
) -> Result<T, StoreError> {
    let query = format!("SELECT {payload_column} FROM {table} WHERE {id_column} = $1");
    let row = client
        .query_opt(&query, &[&id_value])
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| StoreError::NotFound(format!("{table}.{id_column}={id_value}")))?;
    let payload: serde_json::Value = row.get(0);
    serde_json::from_value(payload).map_err(|err| StoreError::InvalidData(err.to_string()))
}

pub(crate) fn geoblock_from_runtime_account_status(status: Option<&str>) -> GeoblockStatus {
    match status.map(|s| s.trim().to_ascii_uppercase()) {
        Some(s) if matches!(s.as_str(), "ACTIVE" | "ALLOWED" | "READY") => GeoblockStatus::Allowed,
        Some(s) if matches!(s.as_str(), "BLOCKED" | "GEO_BLOCKED" | "GEOBLOCKED") => {
            GeoblockStatus::Blocked
        }
        Some(s) if s == "ERROR" => GeoblockStatus::Error,
        _ => GeoblockStatus::Unknown,
    }
}

pub(crate) fn collateral_status_from_db(
    status: Option<&str>,
    explicit_profile: bool,
) -> CollateralProfileStatus {
    match status.map(|s| s.trim().to_ascii_uppercase()) {
        Some(s) if s == "RESOLVED" => CollateralProfileStatus::Resolved,
        Some(s) if matches!(s.as_str(), "DEFAULT" | "DEFAULT_RESOLVED") => {
            CollateralProfileStatus::DefaultResolved
        }
        Some(s) if matches!(s.as_str(), "MISSING" | "EXPLICIT_MISSING") => {
            CollateralProfileStatus::ExplicitMissing
        }
        None if explicit_profile => CollateralProfileStatus::ExplicitMissing,
        _ => CollateralProfileStatus::Unknown,
    }
}

pub(crate) fn worker_status_from_rows(
    rows: &[(String, chrono::DateTime<Utc>)],
    expected: usize,
) -> WorkerStatus {
    if expected == 0 {
        return WorkerStatus::Healthy;
    }
    if rows.len() < expected {
        return WorkerStatus::Unknown;
    }
    let stale_cutoff = Utc::now() - Duration::seconds(120);
    let mut degraded = false;
    for (status, last_heartbeat_at) in rows {
        let normalized = status.trim().to_ascii_uppercase();
        if matches!(normalized.as_str(), "STALE" | "ERROR" | "DOWN")
            || *last_heartbeat_at < stale_cutoff
        {
            return WorkerStatus::Stale;
        }
        if normalized == "DEGRADED" {
            degraded = true;
        } else if normalized != "HEALTHY" {
            return WorkerStatus::Unknown;
        }
    }
    if degraded {
        WorkerStatus::Degraded
    } else {
        WorkerStatus::Healthy
    }
}
