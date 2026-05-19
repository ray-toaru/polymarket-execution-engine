use super::*;

pub async fn apply_schema(client: &Client) -> Result<(), StoreError> {
    let Some((initial, remaining)) = SCHEMA_MIGRATIONS.split_first() else {
        return Ok(());
    };
    let Some((framework, forward_migrations)) = remaining.split_first() else {
        return Err(StoreError::InvalidData(
            "schema migration framework is not configured".into(),
        ));
    };

    client
        .batch_execute(initial.sql)
        .await
        .map_err(map_db_error)?;
    client
        .batch_execute(framework.sql)
        .await
        .map_err(map_db_error)?;
    record::record_applied_migration(client, initial).await?;
    record::record_applied_migration(client, framework).await?;

    for migration in forward_migrations {
        apply_forward_migration(client, migration).await?;
    }
    Ok(())
}

async fn apply_forward_migration(
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
        .batch_execute(migration.sql)
        .await
        .map_err(map_db_error)?;
    record::record_applied_migration(client, migration).await
}
