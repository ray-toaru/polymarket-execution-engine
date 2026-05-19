use super::*;
use tokio_postgres::{Client, types::ToSql};

pub async fn load_runtime_worker_observations(
    client: &Client,
    query: &RuntimeStateQuery,
) -> Result<Vec<RuntimeWorkerObservation>, StoreError> {
    let observation_ttl_seconds: i32 = runtime_observation_ttl_seconds() as i32;
    let params: &[&(dyn ToSql + Sync)] = &[&query.account_id, &observation_ttl_seconds];
    let observation_rows = client
        .query(
            "SELECT DISTINCT ON (capability)
                account_id, capability, worker_kind, status, should_fail_closed, reason, observed_at
             FROM runtime_worker_observations
             WHERE account_id = $1
               AND observed_at >= now() - ($2::integer * interval '1 second')
             ORDER BY capability, observed_at DESC, observation_id DESC",
            params,
        )
        .await
        .map_err(map_db_error)?;
    Ok(observation_rows
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
        .collect())
}
