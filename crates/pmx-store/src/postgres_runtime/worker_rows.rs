use super::*;
use tokio_postgres::GenericClient;

pub async fn load_worker_rows(
    client: &(impl GenericClient + Sync),
    required_capabilities: &[String],
) -> Result<Vec<(String, chrono::DateTime<Utc>)>, StoreError> {
    let mut worker_rows = Vec::new();
    for capability in required_capabilities {
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
    Ok(worker_rows)
}
