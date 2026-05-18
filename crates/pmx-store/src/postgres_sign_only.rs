mod read;
mod write;

use async_trait::async_trait;
use pmx_core::SignOnlyLifecycleRecord;

use crate::postgres::PostgresStore;
use crate::{SignOnlyLifecycleQuery, SignOnlyLifecycleStore, StoreError};

#[async_trait]
impl SignOnlyLifecycleStore for PostgresStore {
    async fn record_sign_only_lifecycle_event(
        &self,
        record: &SignOnlyLifecycleRecord,
    ) -> Result<(), StoreError> {
        write::record_sign_only_lifecycle_event(self, record).await
    }

    async fn list_sign_only_lifecycle_events(
        &self,
        query: &SignOnlyLifecycleQuery,
    ) -> Result<Vec<SignOnlyLifecycleRecord>, StoreError> {
        read::list_sign_only_lifecycle_events(self, query).await
    }
}
