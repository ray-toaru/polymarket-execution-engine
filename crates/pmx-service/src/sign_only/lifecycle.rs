use pmx_core::{SignOnlyLifecycleRecord, sign_only_lifecycle_records_equivalent};
use pmx_store::{SignOnlyLifecycleQuery, SignOnlyLifecycleStore};

use crate::ServiceError;
use crate::binding::validate_sign_only_lifecycle_append;

pub async fn record_sign_only_lifecycle_event<S>(
    store: &S,
    mut record: SignOnlyLifecycleRecord,
) -> Result<SignOnlyLifecycleRecord, ServiceError>
where
    S: SignOnlyLifecycleStore + Send + Sync,
{
    record.event_id = None;
    record.created_at = None;
    let query = SignOnlyLifecycleQuery {
        execution_id: record.execution_id.0.clone(),
        limit: 500,
        before_event_id: None,
    };
    let existing = store.list_sign_only_lifecycle_events(&query).await?;
    validate_sign_only_lifecycle_append(&existing, &record)?;
    store.record_sign_only_lifecycle_event(&record).await?;
    let updated = store.list_sign_only_lifecycle_events(&query).await?;
    let matched = if let Some(client_event_id) = record.client_event_id.as_deref() {
        updated
            .iter()
            .rev()
            .find(|candidate| candidate.client_event_id.as_deref() == Some(client_event_id))
    } else {
        updated
            .iter()
            .rev()
            .find(|candidate| sign_only_lifecycle_records_equivalent(candidate, &record))
    };
    Ok(matched.cloned().unwrap_or(record))
}
