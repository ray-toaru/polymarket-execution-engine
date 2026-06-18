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

    let portfolio_projection = json!({
        "account_id": "acct-http-e2e-1",
        "fills": [{
            "fill_id": "fill-http-e2e-1",
            "order_id": "order-v07-1",
            "token_id": "token-http-e2e-1",
            "side": "BUY",
            "price": "0.50",
            "shares": "2",
            "observed_at_ms": 1_000
        }],
        "positions": [{
            "token_id": "token-http-e2e-1",
            "shares": "2",
            "average_price": "0.50"
        }],
        "open_orders": [{
            "order_id": "order-v07-2",
            "token_id": "token-http-e2e-2",
            "side": "SELL",
            "remaining_shares": "3",
            "limit_price": "0.60"
        }],
        "exposure": {
            "gross_notional": "1.00",
            "open_order_notional": "1.80"
        },
        "observed_at_ms": 2_000
    });
    let (status, recorded_projection) = request_json(
        app.clone(),
        "POST",
        "/v1/portfolio/projections",
        Some("service-token-test-v07"),
        Some(portfolio_projection.clone()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "record portfolio projection: {recorded_projection}"
    );
    assert_eq!(recorded_projection["no_remote_side_effect"], true);

    let (status, loaded_projection) = request_json(
        app.clone(),
        "GET",
        "/v1/portfolio/acct-http-e2e-1/projection",
        Some("service-token-test-v07"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "load portfolio projection: {loaded_projection}"
    );
    assert_eq!(loaded_projection, portfolio_projection);

    let (status, risk_decision) = request_json(
        app.clone(),
        "POST",
        "/v1/portfolio/acct-http-e2e-1/risk-assessments",
        Some("service-token-test-v07"),
        Some(json!({
            "max_gross_notional": "2",
            "max_open_order_notional": "1",
            "kill_switch_active": false
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "risk decision: {risk_decision}");
    assert_eq!(risk_decision["decision"], "BLOCK");
    assert_eq!(risk_decision["reason"], "OPEN_ORDER_EXPOSURE_EXCEEDED");

    let (status, _) = request_json(
        app.clone(),
        "GET",
        "/v1/admin/audit-events?limit=20",
        Some("service-token-test-v07"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = request_json(
        app.clone(),
        "GET",
        "/v1/admin/live-read-events?limit=20&account_id=acct-http-e2e-1&operation=GET_ORDER",
        Some("service-token-test-v07"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, live_read_events) = request_json(
        app.clone(),
        "GET",
        "/v1/admin/live-read-events?limit=20&account_id=acct-http-e2e-1&operation=GET_ORDER",
        Some("admin-read-token-test-v07"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "live-read events: {live_read_events}"
    );
    assert_eq!(live_read_events.as_array().unwrap().len(), 1);
    assert_eq!(live_read_events[0]["no_trading_side_effect"], true);
    assert_eq!(
        live_read_events[0]["redacted_error_summary"],
        json!("remote unknown api_secret=[REDACTED] signature=[REDACTED]")
    );
    assert!(
        live_read_events[0]["redacted_fields"]
            .as_array()
            .unwrap()
            .contains(&json!("api_secret"))
    );

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
