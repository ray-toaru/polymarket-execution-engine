use super::*;
use tokio_postgres::Client;

pub async fn load_account_state(
    client: &Client,
    query: &RuntimeStateQuery,
) -> Result<(pmx_core::GeoblockStatus, bool), StoreError> {
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
    Ok((
        geoblock_from_runtime_account_status(account_status.as_deref()),
        kill_switch_enabled,
    ))
}

pub async fn load_global_kill_switch_enabled(client: &Client) -> Result<bool, StoreError> {
    let row = client
        .query_opt(
            "SELECT enabled FROM runtime_global_controls WHERE control_key = 'kill_switch'",
            &[],
        )
        .await
        .map_err(map_db_error)?;
    Ok(row.map(|row| row.get::<_, bool>(0)).unwrap_or(false))
}

pub async fn set_account_kill_switch(
    client: &Client,
    account_id: &pmx_core::AccountId,
    enabled: bool,
    reason: &str,
) -> Result<KillSwitchStateChange, StoreError> {
    let row = client
        .query_one(
            "INSERT INTO runtime_accounts \
             (account_id, status, kill_switch_enabled, kill_switch_version, kill_switch_reason, kill_switch_updated_at, updated_at) \
             VALUES ($1, 'UNKNOWN', $2, 1, $3, now(), now()) \
             ON CONFLICT (account_id) DO UPDATE SET \
                kill_switch_enabled = EXCLUDED.kill_switch_enabled, \
                kill_switch_version = runtime_accounts.kill_switch_version + 1, \
                kill_switch_reason = EXCLUDED.kill_switch_reason, \
                kill_switch_updated_at = now(), \
                updated_at = now() \
             RETURNING kill_switch_enabled, kill_switch_version, kill_switch_updated_at",
            &[&account_id.0, &enabled, &reason],
        )
        .await
        .map_err(map_db_error)?;
    Ok(KillSwitchStateChange {
        scope: pmx_core::KillSwitchScope::Account,
        account_id: Some(account_id.clone()),
        enabled: row.get::<_, bool>(0),
        state_version: row.get::<_, i64>(1),
        effective_at: row.get(2),
    })
}

pub async fn set_global_kill_switch(
    client: &Client,
    enabled: bool,
    reason: &str,
) -> Result<KillSwitchStateChange, StoreError> {
    let row = client
        .query_one(
            "INSERT INTO runtime_global_controls \
             (control_key, enabled, control_version, reason, updated_at) \
             VALUES ('kill_switch', $1, 1, $2, now()) \
             ON CONFLICT (control_key) DO UPDATE SET \
                enabled = EXCLUDED.enabled, \
                control_version = runtime_global_controls.control_version + 1, \
                reason = EXCLUDED.reason, \
                updated_at = now() \
             RETURNING enabled, control_version, updated_at",
            &[&enabled, &reason],
        )
        .await
        .map_err(map_db_error)?;
    Ok(KillSwitchStateChange {
        scope: pmx_core::KillSwitchScope::Global,
        account_id: None,
        enabled: row.get::<_, bool>(0),
        state_version: row.get::<_, i64>(1),
        effective_at: row.get(2),
    })
}

pub async fn load_collateral_profile_status(
    client: &Client,
    query: &RuntimeStateQuery,
) -> Result<pmx_core::CollateralProfileStatus, StoreError> {
    if let Some(profile_id) = &query.collateral_profile_id {
        let row = client
            .query_opt(
                "SELECT status FROM collateral_profiles WHERE profile_id = $1",
                &[profile_id],
            )
            .await
            .map_err(map_db_error)?;
        let status = row.map(|row| row.get::<_, String>(0));
        Ok(collateral_status_from_db(status.as_deref(), true))
    } else {
        let row = client
            .query_opt(
                "SELECT status FROM collateral_profiles WHERE status IN ('DEFAULT', 'DEFAULT_RESOLVED', 'RESOLVED') ORDER BY created_at DESC LIMIT 1",
                &[],
            )
            .await
            .map_err(map_db_error)?;
        let status = row.map(|row| row.get::<_, String>(0));
        Ok(collateral_status_from_db(status.as_deref(), false))
    }
}
