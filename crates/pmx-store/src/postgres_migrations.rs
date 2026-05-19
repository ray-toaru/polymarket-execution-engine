use crate::StoreError;
use tokio_postgres::Client;

use super::map_db_error;

#[path = "postgres_migrations/apply.rs"]
mod apply;

#[path = "postgres_migrations/checksum.rs"]
mod checksum;

#[path = "postgres_migrations/manifest.rs"]
mod manifest;

#[path = "postgres_migrations/record.rs"]
mod record;

pub(super) use checksum::sha256_hex;
pub(super) use manifest::{SCHEMA_MIGRATIONS, SchemaMigration};

pub(super) async fn apply_schema(client: &Client) -> Result<(), StoreError> {
    apply::apply_schema(client).await
}
