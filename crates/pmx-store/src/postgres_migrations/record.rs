use super::*;

pub async fn record_applied_migration(
    client: &Client,
    migration: &SchemaMigration,
) -> Result<(), StoreError> {
    let checksum = sha256_hex(migration.sql);
    if let Some(row) = client
        .query_opt(
            "SELECT checksum_sha256 FROM schema_migrations WHERE version = $1",
            &[&migration.version],
        )
        .await
        .map_err(map_db_error)?
    {
        let existing: String = row.get(0);
        if existing != checksum {
            return Err(StoreError::Conflict(format!(
                "schema migration checksum mismatch for {}",
                migration.version
            )));
        }
        return Ok(());
    }
    client
        .execute(
            "INSERT INTO schema_migrations (version, checksum_sha256) VALUES ($1, $2)",
            &[&migration.version, &checksum],
        )
        .await
        .map_err(map_db_error)?;
    Ok(())
}
