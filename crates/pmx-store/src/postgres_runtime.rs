use async_trait::async_trait;
use chrono::Utc;
use pmx_core::RuntimeStateSummary;
use tokio_postgres::IsolationLevel;

use crate::postgres::PostgresStore;
use crate::postgres_support::{
    collateral_status_from_db, geoblock_from_runtime_account_status, map_db_error,
    worker_status_from_rows,
};
use crate::{
    KillSwitchStateChange, RuntimeControlStore, RuntimeStateQuery, RuntimeStateStore,
    RuntimeWorkerObservation, StoreError, apply_runtime_worker_observations,
    runtime_observation_ttl_seconds,
};

#[path = "postgres_runtime/account_collateral.rs"]
mod account_collateral;

#[path = "postgres_runtime/observations.rs"]
mod observations;

#[path = "postgres_runtime/worker_rows.rs"]
mod worker_rows;

#[async_trait]
impl RuntimeStateStore for PostgresStore {
    async fn load_runtime_state(
        &self,
        query: &RuntimeStateQuery,
    ) -> Result<RuntimeStateSummary, StoreError> {
        let mut client = self.client().await?;
        let transaction = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(map_db_error)?;
        let (geoblock_status, account_kill_switch_enabled) =
            account_collateral::load_account_state(&transaction, query).await?;
        let global_kill_switch_enabled =
            account_collateral::load_global_kill_switch_enabled(&transaction).await?;
        let collateral_profile_status =
            account_collateral::load_collateral_profile_status(&transaction, query).await?;

        let mut required_capabilities = query.required_capabilities.clone();
        if required_capabilities.is_empty() {
            required_capabilities = vec![
                "heartbeat".into(),
                "reconcile".into(),
                "resource-refresh".into(),
            ];
        }
        let worker_rows =
            worker_rows::load_worker_rows(&transaction, &required_capabilities).await?;
        let base = RuntimeStateSummary {
            geoblock_status,
            worker_status: worker_status_from_rows(&worker_rows, required_capabilities.len()),
            collateral_profile_status,
            kill_switch_enabled: account_kill_switch_enabled || global_kill_switch_enabled,
            required_capabilities,
        };
        let observations =
            observations::load_runtime_worker_observations(&transaction, query).await?;
        transaction.commit().await.map_err(map_db_error)?;
        Ok(apply_runtime_worker_observations(base, &observations))
    }
}

#[async_trait]
impl RuntimeControlStore for PostgresStore {
    async fn set_account_kill_switch(
        &self,
        account_id: &pmx_core::AccountId,
        enabled: bool,
        reason: &str,
    ) -> Result<KillSwitchStateChange, StoreError> {
        let client = self.client().await?;
        account_collateral::set_account_kill_switch(&client, account_id, enabled, reason).await
    }

    async fn set_global_kill_switch(
        &self,
        enabled: bool,
        reason: &str,
    ) -> Result<KillSwitchStateChange, StoreError> {
        let client = self.client().await?;
        account_collateral::set_global_kill_switch(&client, enabled, reason).await
    }
}
