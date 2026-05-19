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
