use super::super::*;

#[tokio::test]
async fn in_memory_admin_audit_records_without_exposing_secrets() {
    let store = InMemoryStore::default();
    store
        .record_admin_audit_event(&AdminAuditEvent {
            audit_id: None,
            principal_subject: "admin-token".into(),
            operation: "KillSwitch".into(),
            request_fingerprint: Some("abc123".into()),
            correlation_id: Some("corr-admin-test".into()),
            result: "ACCEPTED".into(),
            created_at: None,
        })
        .await
        .expect("record audit event");
    let len = store
        .inner
        .lock()
        .expect("in-memory store mutex poisoned")
        .admin_audit
        .len();
    assert_eq!(len, 1);
}

#[tokio::test]
async fn in_memory_admin_audit_paginates_and_filters_by_cursor() {
    let store = InMemoryStore::default();
    for (operation, correlation_id, result) in [
        ("KillSwitch", "corr-audit-page-1", "ACCEPTED"),
        ("RuntimeOverride", "corr-audit-page-2", "DENIED"),
        ("KillSwitch", "corr-audit-page-3", "ACCEPTED"),
    ] {
        store
            .record_admin_audit_event(&AdminAuditEvent {
                audit_id: None,
                principal_subject: "admin-page-test".into(),
                operation: operation.into(),
                request_fingerprint: Some(format!("fp-{correlation_id}")),
                correlation_id: Some(correlation_id.into()),
                result: result.into(),
                created_at: None,
            })
            .await
            .expect("record audit page event");
    }

    let first_page = store
        .list_admin_audit_events(&AdminAuditQuery {
            limit: 2,
            principal_subject: Some("admin-page-test".into()),
            ..AdminAuditQuery::default()
        })
        .await
        .expect("first page");
    assert_eq!(first_page.len(), 2);
    assert_eq!(
        first_page
            .iter()
            .map(|event| event.correlation_id.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("corr-audit-page-2"), Some("corr-audit-page-3")]
    );

    let older_page = store
        .list_admin_audit_events(&AdminAuditQuery {
            limit: 2,
            before_audit_id: first_page[0].audit_id,
            principal_subject: Some("admin-page-test".into()),
            ..AdminAuditQuery::default()
        })
        .await
        .expect("older page");
    assert_eq!(older_page.len(), 1);
    assert_eq!(
        older_page[0].correlation_id.as_deref(),
        Some("corr-audit-page-1")
    );

    let filtered = store
        .list_admin_audit_events(&AdminAuditQuery {
            limit: 10,
            operation: Some("KillSwitch".into()),
            result: Some("ACCEPTED".into()),
            correlation_id: Some("corr-audit-page-3".into()),
            principal_subject: Some("admin-page-test".into()),
            ..AdminAuditQuery::default()
        })
        .await
        .expect("filtered page");
    assert_eq!(filtered.len(), 1);
    assert_eq!(
        filtered[0].correlation_id.as_deref(),
        Some("corr-audit-page-3")
    );
}
