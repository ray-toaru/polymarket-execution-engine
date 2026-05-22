use super::*;
use tokio_postgres::GenericClient;

pub async fn load_worker_rows(
    client: &(impl GenericClient + Sync),
    query: &RuntimeStateQuery,
    required_capabilities: &[String],
) -> Result<Vec<(String, chrono::DateTime<Utc>)>, StoreError> {
    let mut worker_rows = Vec::new();
    for capability in required_capabilities {
        if let Some(row) = client
            .query_opt(
                "SELECT status, last_heartbeat_at FROM worker_health
                 WHERE capability = $1
                   AND (account_id IS NULL OR account_id = $2)
                   AND (condition_id IS NULL OR condition_id = $3)
                 ORDER BY
                   CASE WHEN account_id = $2 THEN 0 ELSE 1 END,
                   CASE WHEN condition_id = $3 THEN 0 ELSE 1 END,
                   updated_at DESC
                 LIMIT 1",
                &[capability, &query.account_id, &query.condition_id],
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
    Ok(worker_rows)
}
