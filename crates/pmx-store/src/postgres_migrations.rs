use crate::StoreError;
use sha2::{Digest, Sha256};
use tokio_postgres::Client;

use super::map_db_error;

struct SchemaMigration {
    version: &'static str,
    sql: &'static str,
}

const SCHEMA_MIGRATIONS: &[SchemaMigration] = &[
    SchemaMigration {
        version: "0001_initial",
        sql: include_str!("../../../migrations/0001_initial.sql"),
    },
    SchemaMigration {
        version: "0002_migration_framework",
        sql: include_str!("../../../migrations/0002_migration_framework.sql"),
    },
    SchemaMigration {
        version: "0003_order_event_trace",
        sql: include_str!("../../../migrations/0003_order_event_trace.sql"),
    },
];

pub(super) async fn apply_schema(client: &Client) -> Result<(), StoreError> {
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
    record_applied_migration(client, initial).await?;
    record_applied_migration(client, framework).await?;

    for migration in forward_migrations {
        apply_forward_migration(client, migration).await?;
    }
    Ok(())
}

async fn record_applied_migration(
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
    record_applied_migration(client, migration).await
}

fn sha256_hex(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
