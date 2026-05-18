use pmx_core::SignOnlyLifecycleRecord;

use crate::{AdminAuditEvent, ExecutionLifecycleEvent};

pub(crate) fn sanitize_admin_audit_event(mut event: AdminAuditEvent) -> AdminAuditEvent {
    event.audit_id = None;
    event.created_at = None;
    event
}

pub(crate) fn sanitize_execution_lifecycle_event(
    mut event: ExecutionLifecycleEvent,
) -> ExecutionLifecycleEvent {
    event.event_id = None;
    event.created_at = None;
    event
}

pub(crate) fn sanitize_sign_only_lifecycle_record(
    mut record: SignOnlyLifecycleRecord,
) -> SignOnlyLifecycleRecord {
    record.event_id = None;
    record.created_at = None;
    record
}
