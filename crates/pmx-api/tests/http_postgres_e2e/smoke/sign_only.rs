use super::super::*;

pub(super) async fn verify_sign_only_flow(
    app: axum::Router,
    execution_id: &str,
    plan_hash: &str,
    suffix: &str,
) {
    let (status, standard_sign_only) = request_json(
        app.clone(),
        "POST",
        "/v1/sign-only/standard-constructions",
        Some("service-token-pg-e2e"),
        Some(json!({
            "execution_id": execution_id,
            "account_id": format!("acct-http-pg-e2e-{suffix}"),
            "plan_hash": plan_hash,
            "no_remote_side_effect": true
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "standard sign-only PG response: {standard_sign_only}"
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
        app,
        "GET",
        &sign_only_uri,
        Some("service-token-pg-e2e"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "sign-only PG list: {sign_only_records}"
    );
    assert_eq!(sign_only_records.as_array().unwrap().len(), 3);
}
