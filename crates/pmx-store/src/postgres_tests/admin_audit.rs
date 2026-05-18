use super::*;

#[tokio::test]
async fn postgres_records_admin_audit_event() {
    let Some(store) = test_store().await else {
        return;
    };
    let principal = unique("principal");
    store
        .record_admin_audit_event(&AdminAuditEvent {
            audit_id: None,
            principal_subject: principal.clone(),
            operation: "KillSwitch".into(),
            request_fingerprint: Some(unique("request-fp")),
            correlation_id: Some(unique("corr")),
            result: "ACCEPTED".into(),
            created_at: None,
        })
        .await
        .expect("record audit event");
    let client = store.client().await.expect("test postgres client");
    let row = client
        .query_one(
            "SELECT COUNT(*)::bigint FROM admin_audit_events WHERE principal_subject = $1",
            &[&principal],
        )
        .await
        .expect("count audit events");
    let count: i64 = row.get(0);
    assert_eq!(count, 1);
}

#[tokio::test]
async fn postgres_admin_audit_paginates_and_filters_by_cursor() {
    let Some(store) = test_store().await else {
        return;
    };
    let principal = unique("principal-page");
    let corr_1 = unique("corr-audit-page-1");
    let corr_2 = unique("corr-audit-page-2");
    let corr_3 = unique("corr-audit-page-3");
    for (operation, correlation_id, result) in [
        ("KillSwitch", corr_1.clone(), "ACCEPTED"),
        ("RuntimeOverride", corr_2.clone(), "DENIED"),
        ("KillSwitch", corr_3.clone(), "ACCEPTED"),
    ] {
        store
            .record_admin_audit_event(&AdminAuditEvent {
                audit_id: None,
                principal_subject: principal.clone(),
                operation: operation.into(),
                request_fingerprint: Some(unique("request-fp-page")),
                correlation_id: Some(correlation_id),
                result: result.into(),
                created_at: None,
            })
            .await
            .expect("record audit page event");
    }

    let first_page = store
        .list_admin_audit_events(&AdminAuditQuery {
            limit: 2,
            principal_subject: Some(principal.clone()),
            ..AdminAuditQuery::default()
        })
        .await
        .expect("first page");
    assert_eq!(first_page.len(), 2);
    assert_eq!(
        first_page
            .iter()
            .map(|event| event.correlation_id.clone())
            .collect::<Vec<_>>(),
        vec![Some(corr_2.clone()), Some(corr_3.clone())]
    );

    let older_page = store
        .list_admin_audit_events(&AdminAuditQuery {
            limit: 2,
            before_audit_id: first_page[0].audit_id,
            principal_subject: Some(principal.clone()),
            ..AdminAuditQuery::default()
        })
        .await
        .expect("older page");
    assert_eq!(older_page.len(), 1);
    assert_eq!(older_page[0].correlation_id, Some(corr_1));

    let filtered = store
        .list_admin_audit_events(&AdminAuditQuery {
            limit: 10,
            operation: Some("KillSwitch".into()),
            result: Some("ACCEPTED".into()),
            correlation_id: Some(corr_3.clone()),
            principal_subject: Some(principal),
            ..AdminAuditQuery::default()
        })
        .await
        .expect("filtered page");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].correlation_id, Some(corr_3));
}
