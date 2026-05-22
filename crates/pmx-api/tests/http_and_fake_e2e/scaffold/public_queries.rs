use super::super::*;

pub(super) async fn verify_public_queries(app: axum::Router, execution_id: &str) {
    let lifecycle_uri = format!("/v1/lifecycle/executions/{execution_id}/events");
    let (status, lifecycle_events) = request_json(
        app.clone(),
        "GET",
        &lifecycle_uri,
        Some("service-token-test-v07"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "lifecycle events: {lifecycle_events}"
    );
    let event_types: Vec<_> = lifecycle_events
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["event_type"].as_str().unwrap().to_string())
        .collect();
    assert!(event_types.contains(&"CANCEL_REQUESTED_NON_LIVE".to_string()));
    assert!(event_types.contains(&"RECONCILE_REQUESTED_NON_LIVE".to_string()));
    for event in lifecycle_events.as_array().unwrap() {
        if matches!(
            event["event_type"].as_str().unwrap(),
            "CANCEL_REQUESTED_NON_LIVE" | "RECONCILE_REQUESTED_NON_LIVE"
        ) {
            assert_eq!(event["payload"]["schema_version"], 1);
            assert!(event["payload"]["correlation_id"].as_str().is_some());
            assert_eq!(event["payload"]["body"]["no_remote_side_effect"], true);
            assert!(
                event["payload"]["redacted_fields"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("signed_payload"))
            );
        }
    }

    let (status, order_events) = request_json(
        app.clone(),
        "GET",
        "/v1/lifecycle/orders/order-v07-1/events",
        Some("service-token-test-v07"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "order events: {order_events}");
    let order_event_types: Vec<_> = order_events
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["event"].as_str().unwrap().to_string())
        .collect();
    assert!(order_event_types.contains(&"CANCEL_REQUESTED".to_string()));
    assert!(
        order_events
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["payload"]["no_remote_side_effect"] == true)
    );

    let (status, runtime_workers) = request_json(
        app.clone(),
        "GET",
        "/v1/runtime/workers?account_id=acct-http-e2e-1&limit=20",
        Some("service-token-test-v07"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "runtime workers: {runtime_workers}");
    assert!(runtime_workers["heartbeats"].as_array().unwrap().is_empty());
    assert!(
        runtime_workers["observations"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let (status, _) = request_json(
        app.clone(),
        "GET",
        "/v1/admin/audit-events?limit=20",
        Some("service-token-test-v07"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, audit_events) = request_json(
        app,
        "GET",
        "/v1/admin/audit-events?limit=20",
        Some("admin-token-test-v07"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "audit events: {audit_events}");
    assert!(audit_events.as_array().unwrap().len() >= 2);
}
