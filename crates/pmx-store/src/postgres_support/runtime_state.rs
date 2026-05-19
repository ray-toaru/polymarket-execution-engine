use super::*;

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
