#[path = "standard/digest.rs"]
mod digest;

#[path = "standard/persist.rs"]
mod persist;

#[path = "standard/validate.rs"]
mod validate;

use pmx_store::{ExecutionStore, SignOnlyLifecycleQuery, SignOnlyLifecycleStore};

use crate::sign_only::record_sign_only_lifecycle_event;
use crate::{
    ServiceError, StandardSignOnlyConstructionReceipt, StandardSignOnlyConstructionRequest,
};

pub async fn record_standard_sign_only_construction<S>(
    store: &S,
    req: StandardSignOnlyConstructionRequest,
) -> Result<StandardSignOnlyConstructionReceipt, ServiceError>
where
    S: ExecutionStore + SignOnlyLifecycleStore + Send + Sync,
{
    validate::validate_standard_sign_only_request(&req)?;
    let plan = store.load_plan_summary(&req.execution_id).await?;
    validate::validate_standard_sign_only_plan_match(&plan, &req)?;

    let signed_order_digest = digest::resolve_signed_order_digest(&plan, req.signed_order_digest)?;
    let signed_order_ref = digest::resolve_signed_order_ref(
        &req.execution_id,
        &req.plan_hash,
        req.signed_order_ref,
        &signed_order_digest,
    );

    let query = SignOnlyLifecycleQuery {
        execution_id: req.execution_id.clone(),
        limit: 500,
        before_event_id: None,
    };
    let existing = store.list_sign_only_lifecycle_events(&query).await?;
    if let Some(replay_records) = persist::try_replay_standard_sign_only(&existing, &req.plan_hash)
    {
        return Ok(StandardSignOnlyConstructionReceipt {
            execution_id: req.execution_id,
            signed_order_ref,
            signed_order_digest: Some(signed_order_digest),
            lifecycle_records: replay_records,
            no_remote_side_effect: true,
        });
    }

    let mut lifecycle_records = Vec::with_capacity(persist::standard_stages().len());
    for record in persist::build_standard_sign_only_records(
        &req.execution_id,
        &req.account_id,
        &req.plan_hash,
        &signed_order_ref,
    ) {
        let record = record_sign_only_lifecycle_event(store, record).await?;
        lifecycle_records.push(record);
    }

    Ok(StandardSignOnlyConstructionReceipt {
        execution_id: req.execution_id,
        signed_order_ref,
        signed_order_digest: Some(signed_order_digest),
        lifecycle_records,
        no_remote_side_effect: true,
    })
}
