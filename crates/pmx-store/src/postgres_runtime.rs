use async_trait::async_trait;
use chrono::Utc;
use pmx_core::RuntimeStateSummary;

use crate::postgres::PostgresStore;
use crate::postgres_support::{
    collateral_status_from_db, geoblock_from_runtime_account_status, map_db_error,
    worker_status_from_rows,
};
use crate::{
    RuntimeStateQuery, RuntimeStateStore, RuntimeWorkerObservation, StoreError,
    apply_runtime_worker_observations, runtime_observation_ttl_seconds,
};

#[async_trait]
impl RuntimeStateStore for PostgresStore {
    async fn load_runtime_state(
        &self,
        query: &RuntimeStateQuery,
    ) -> Result<RuntimeStateSummary, StoreError> {
        let client = self.client().await?;
        let account_row = client
            .query_opt(
                "SELECT status, kill_switch_enabled FROM runtime_accounts WHERE account_id = $1",
                &[&query.account_id],
            )
            .await
            .map_err(map_db_error)?;
        let (account_status, kill_switch_enabled) = if let Some(row) = account_row {
            (Some(row.get::<_, String>(0)), row.get::<_, bool>(1))
        } else {
            (None, true)
        };

        let geoblock_status = geoblock_from_runtime_account_status(account_status.as_deref());

        let collateral_profile_status = if let Some(profile_id) = &query.collateral_profile_id {
            let row = client
                .query_opt(
                    "SELECT status FROM collateral_profiles WHERE profile_id = $1",
                    &[profile_id],
                )
                .await
                .map_err(map_db_error)?;
            let status = row.map(|row| row.get::<_, String>(0));
            collateral_status_from_db(status.as_deref(), true)
        } else {
            let row = client
                .query_opt(
                    "SELECT status FROM collateral_profiles WHERE status IN ('DEFAULT', 'DEFAULT_RESOLVED', 'RESOLVED') ORDER BY created_at DESC LIMIT 1",
                    &[],
                )
                .await
                .map_err(map_db_error)?;
            let status = row.map(|row| row.get::<_, String>(0));
            collateral_status_from_db(status.as_deref(), false)
        };

        let mut required_capabilities = query.required_capabilities.clone();
        if required_capabilities.is_empty() {
            required_capabilities = vec![
                "heartbeat".into(),
                "reconcile".into(),
                "resource-refresh".into(),
            ];
        }
        let mut worker_rows = Vec::new();
        for capability in &required_capabilities {
            if let Some(row) = client
                .query_opt(
                    "SELECT status, last_heartbeat_at FROM worker_health WHERE capability = $1 ORDER BY updated_at DESC LIMIT 1",
                    &[capability],
                )
                .await
                .map_err(map_db_error)?
            {
                worker_rows.push((
                    row.get::<_, String>(0),
                    row.get::<_, chrono::DateTime<Utc>>(1),
                ));
            }
        }

        let base = RuntimeStateSummary {
            geoblock_status,
            worker_status: worker_status_from_rows(&worker_rows, required_capabilities.len()),
            collateral_profile_status,
            kill_switch_enabled,
            required_capabilities,
        };
        let observation_ttl_seconds: i32 = runtime_observation_ttl_seconds() as i32;
        let observation_rows = client
            .query(
                "SELECT DISTINCT ON (capability)
                    account_id, capability, worker_kind, status, should_fail_closed, reason, observed_at
                 FROM runtime_worker_observations
                 WHERE account_id = $1
                   AND observed_at >= now() - ($2::integer * interval '1 second')
                 ORDER BY capability, observed_at DESC, observation_id DESC",
                &[&query.account_id, &observation_ttl_seconds],
            )
            .await
            .map_err(map_db_error)?;
        let observations: Vec<RuntimeWorkerObservation> = observation_rows
            .into_iter()
            .map(|row| RuntimeWorkerObservation {
                account_id: row.get(0),
                capability: row.get(1),
                worker_kind: row.get(2),
                status: row.get(3),
                should_fail_closed: row.get(4),
                reason: row.get(5),
                observed_at: Some(row.get(6)),
            })
            .collect();
        Ok(apply_runtime_worker_observations(base, &observations))
    }
}
