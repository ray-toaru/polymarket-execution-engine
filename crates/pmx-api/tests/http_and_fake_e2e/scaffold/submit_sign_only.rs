use super::super::*;

pub(super) async fn verify_submit_and_sign_only(
    app: axum::Router,
    execution_id: &str,
    plan_hash: &str,
) {
    let (status, submit) = request_json(
        app.clone(),
        "POST",
        "/v1/submissions",
        Some("service-token-test-v07"),
        Some(json!({
            "execution_id": execution_id,
            "plan_hash": plan_hash,
            "idempotency_key": "idem-v07-1"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "submit response: {submit}");
    assert_eq!(submit["status"], "BLOCKED");

    let submission_uri = format!(
        "/v1/submissions/{}",
        submit["execution_id"].as_str().unwrap()
    );
    let (status, submission) = request_json(
        app.clone(),
        "GET",
        &submission_uri,
        Some("service-token-test-v07"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "submission response: {submission}");

    let (status, standard_sign_only) = request_json(
        app.clone(),
        "POST",
        "/v1/sign-only/standard-constructions",
        Some("service-token-test-v07"),
        Some(json!({
            "execution_id": execution_id,
            "account_id": "acct-http-e2e-1",
            "plan_hash": plan_hash,
            "no_remote_side_effect": true
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "standard sign-only response: {standard_sign_only}"
    );
    assert_eq!(standard_sign_only["no_remote_side_effect"], true);
    assert!(
        standard_sign_only["signed_order_ref"]
            .as_str()
            .unwrap()
            .starts_with(&format!("sign-only:{execution_id}:"))
    );
    assert_eq!(
        standard_sign_only["signed_order_digest"]
            .as_str()
            .unwrap()
            .len(),
        64
    );

    let sign_only_uri = format!("/v1/sign-only/lifecycle-events/{execution_id}");
    let (status, sign_only_records) = request_json(
        app.clone(),
        "GET",
        &sign_only_uri,
        Some("service-token-test-v07"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "sign-only list: {sign_only_records}"
    );
    assert_eq!(sign_only_records.as_array().unwrap().len(), 3);
    assert_eq!(sign_only_records[2]["state"], "SIGNED_DRY_RUN");

    let (status, invalid_sign_only) = request_json(
        app,
        "POST",
        "/v1/sign-only/lifecycle-events",
        Some("service-token-test-v07"),
        Some(json!({
            "execution_id": execution_id,
            "account_id": "acct-http-e2e-1",
            "state": "SIGNED_DRY_RUN",
            "event": "SIGNED_WITHOUT_POST",
            "signed_order_ref": "signed-order-ref-v23-replay",
            "no_remote_side_effect": false
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "unsafe sign-only lifecycle response: {invalid_sign_only}"
    );
}
