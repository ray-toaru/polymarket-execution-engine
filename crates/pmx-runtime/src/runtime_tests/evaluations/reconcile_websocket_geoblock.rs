use super::super::*;

#[test]
fn reconcile_backlog_evaluation_blocks_submit_for_remote_unknown_orders() {
    let evaluation = evaluate_reconcile_backlog(ReconcileBacklogEvaluationInput {
        remote_unknown_order_ids: vec!["order-1".into(), "order-2".into()],
        observed_at: Utc::now(),
    });
    assert_eq!(evaluation.remote_unknown_orders, 2);
    assert!(evaluation.submit_blocked);
    assert_eq!(evaluation.reason, "remote_unknown_orders=2");
}

#[test]
fn reconcile_backlog_evaluation_allows_submit_when_backlog_empty() {
    let evaluation = evaluate_reconcile_backlog(ReconcileBacklogEvaluationInput {
        remote_unknown_order_ids: vec![],
        observed_at: Utc::now(),
    });
    assert_eq!(evaluation.remote_unknown_orders, 0);
    assert!(!evaluation.submit_blocked);
    assert_eq!(evaluation.reason, "no remote unknown reconcile backlog");
}

#[test]
fn websocket_liveness_evaluation_accepts_fresh_market_and_user_channels() {
    let observed_at = Utc::now();
    let evaluation = evaluate_websocket_liveness(WebSocketLivenessEvaluationInput {
        observed_at,
        stale_after_seconds: 30,
        observations: vec![
            WebSocketLivenessObservation {
                channel: WebSocketChannel::Market,
                connected: true,
                last_message_at: Some(observed_at - chrono::Duration::seconds(5)),
                status: HealthLevel::Healthy,
                last_error: None,
            },
            WebSocketLivenessObservation {
                channel: WebSocketChannel::User,
                connected: true,
                last_message_at: Some(observed_at - chrono::Duration::seconds(10)),
                status: HealthLevel::Healthy,
                last_error: None,
            },
        ],
    });
    assert!(evaluation.market_connected);
    assert!(!evaluation.market_stale);
    assert!(evaluation.user_connected);
    assert!(!evaluation.user_stale);
    assert!(evaluation.missing_channels.is_empty());
}

#[test]
fn websocket_liveness_evaluation_fails_closed_for_disconnected_stale_or_missing_channels() {
    let observed_at = Utc::now();
    let evaluation = evaluate_websocket_liveness(WebSocketLivenessEvaluationInput {
        observed_at,
        stale_after_seconds: 30,
        observations: vec![WebSocketLivenessObservation {
            channel: WebSocketChannel::Market,
            connected: true,
            last_message_at: Some(observed_at - chrono::Duration::seconds(31)),
            status: HealthLevel::Healthy,
            last_error: None,
        }],
    });
    assert!(evaluation.market_connected);
    assert!(evaluation.market_stale);
    assert!(!evaluation.user_connected);
    assert!(evaluation.user_stale);
    assert_eq!(evaluation.missing_channels, vec!["user"]);
}

#[test]
fn geoblock_evaluation_only_allows_explicit_allowed_status() {
    let allowed = evaluate_geoblock_status(GeoblockEvaluationInput {
        status: GeoblockStatus::Allowed,
        observed_at: Utc::now(),
        last_error: None,
    });
    assert!(allowed.submit_allowed);

    let blocked = evaluate_geoblock_status(GeoblockEvaluationInput {
        status: GeoblockStatus::Unknown,
        observed_at: Utc::now(),
        last_error: Some("provider timeout".into()),
    });
    assert!(!blocked.submit_allowed);
    assert_eq!(blocked.reason, "provider timeout");
}
