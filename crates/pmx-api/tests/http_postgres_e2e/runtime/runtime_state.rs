use super::super::*;

pub(super) async fn verify_ready_and_degraded_runtime(
    app: axum::Router,
    database_url: &str,
    account_id: &str,
    intent: Value,
) -> (Value, Value, Value) {
    let (status, normalized) = request_json(
        app.clone(),
        "POST",
        "/v1/intents/normalize",
        Some("service-token-pg-runtime"),
        Some(intent),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "normalize response: {normalized}");

    let (status, snapshot) = request_json(
        app.clone(),
        "POST",
        "/v1/snapshots/capture",
        Some("service-token-pg-runtime"),
        Some(normalized.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "snapshot response: {snapshot}");
    assert_eq!(snapshot["runtime_state"]["geoblock_status"], "ALLOWED");
    assert_eq!(snapshot["runtime_state"]["worker_status"], "HEALTHY");

    seed_runtime_worker_observation(
        database_url,
        account_id,
        "heartbeat",
        "STALE",
        true,
        "heartbeat lease expired",
    )
    .await;
    let runtime_workers_uri = format!("/v1/runtime/workers?account_id={account_id}&limit=20");
    let (status, runtime_workers) = request_json(
        app.clone(),
        "GET",
        &runtime_workers_uri,
        Some("service-token-pg-runtime"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "runtime worker status response: {runtime_workers}"
    );
    assert!(
        runtime_workers["heartbeats"]
            .as_array()
            .unwrap()
            .iter()
            .any(|heartbeat| heartbeat["capability"] == "heartbeat")
    );
    assert!(
        runtime_workers["observations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|observation| observation["status"] == "STALE")
    );
    let (status, degraded_snapshot) = request_json(
        app.clone(),
        "POST",
        "/v1/snapshots/capture",
        Some("service-token-pg-runtime"),
        Some(normalized.clone()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "degraded snapshot response: {degraded_snapshot}"
    );
    assert_eq!(degraded_snapshot["runtime_state"]["worker_status"], "STALE");
    assert!(
        degraded_snapshot["runtime_state"]["required_capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "heartbeat")
    );

    let (status, degraded_decision) = request_json(
        app.clone(),
        "POST",
        "/v1/decisions/evaluate",
        Some("service-token-pg-runtime"),
        Some(json!({"normalized_intent_id": normalized["normalized_intent_id"], "snapshot_id": degraded_snapshot["snapshot_id"]})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "degraded decision response: {degraded_decision}"
    );
    assert_eq!(degraded_decision["status"], "BLOCK");

    let (status, decision) = request_json(
        app,
        "POST",
        "/v1/decisions/evaluate",
        Some("service-token-pg-runtime"),
        Some(json!({"normalized_intent_id": normalized["normalized_intent_id"], "snapshot_id": snapshot["snapshot_id"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "decision response: {decision}");
    assert_eq!(decision["status"], "ALLOW");

    (normalized, snapshot, decision)
}
