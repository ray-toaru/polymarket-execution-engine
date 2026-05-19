use super::*;

#[test]
fn standard_sign_only_profile_is_non_posting_v2_pusd() {
    let profile = OfficialSdkStandardSignOnlyProfile::default();
    validate_standard_sign_only_profile(&profile).expect("standard sign-only profile");
    assert_eq!(profile.clob_host, CLOB_V2_HOST);
    assert_eq!(profile.collateral_symbol, "pUSD");
    assert_eq!(profile.signing_protocol, "CLOB_V2");
    assert!(profile.uses_deposit_wallet_order_path);
    assert!(!profile.exposes_raw_signed_order);
    assert!(!profile.may_post_order);
    assert!(!profile.may_cancel_order);
}

#[test]
fn standard_sign_only_plan_is_default_sdk_construct_path_without_raw_payload() {
    let plan = standard_sign_only_default_plan_for_order(&sample_plan_limit())
        .expect("standard sign-only plan");
    assert_eq!(plan.signed_order_ref_namespace, "sign-only");
    assert_eq!(plan.mapping.order_kind, "LIMIT");
    assert_eq!(plan.mapping.time_in_force.as_deref(), Some("GTC"));
    assert_eq!(plan.profile.clob_host, CLOB_V2_HOST);
    assert_eq!(plan.profile.collateral_symbol, CLOB_V2_COLLATERAL_SYMBOL);
    assert!(plan.profile.uses_deposit_wallet_order_path);
    assert!(!plan.exposes_raw_signed_order);
    assert!(!plan.may_post_order);
    assert!(!plan.may_cancel_order);
}

#[test]
fn standard_sign_only_plan_supports_market_mapping_without_raw_payload() {
    let mut order = sample_plan_limit();
    order.order_kind = "MARKET".into();
    order.limit_price = None;
    order.size = None;
    order.amount = Some("12.5".into());
    order.time_in_force = None;

    let plan = standard_sign_only_default_plan_for_order(&order).expect("market sign-only plan");
    assert_eq!(plan.mapping.order_kind, "MARKET");
    assert_eq!(plan.mapping.amount.as_deref(), Some("12.5"));
    assert!(plan.mapping.limit_price.is_none());
    assert!(plan.mapping.size.is_none());
    assert!(!plan.exposes_raw_signed_order);
    assert!(!plan.may_post_order);
    assert!(!plan.may_cancel_order);
}

#[test]
fn standard_sign_only_construction_emits_only_digest_ref_and_lifecycle() {
    let construction = standard_sign_only_construction_for_order(
        &sample_plan_limit(),
        HashValue("plan-hash-standard".into()),
    )
    .expect("standard sign-only construction");
    assert!(construction.no_remote_side_effect);
    assert!(!construction.raw_signed_order_exposed);
    assert!(!construction.signed_order_digest.is_empty());
    assert!(
        construction
            .signed_order_ref
            .starts_with("sign-only:exec-1:plan-hash-standard:digest-")
    );
    assert_eq!(construction.lifecycle_records.len(), 3);
    assert_eq!(
        construction.lifecycle_records.last().unwrap().state,
        SignOnlyLifecycleState::SignedDryRun
    );
    assert_eq!(
        construction
            .lifecycle_records
            .last()
            .unwrap()
            .signed_order_ref
            .as_deref(),
        Some(construction.signed_order_ref.as_str())
    );
}

#[test]
fn standard_sign_only_construction_ref_is_stable_for_same_mapping() {
    let first = standard_sign_only_construction_for_order(
        &sample_plan_limit(),
        HashValue("plan-hash-stable".into()),
    )
    .expect("first construction");
    let second = standard_sign_only_construction_for_order(
        &sample_plan_limit(),
        HashValue("plan-hash-stable".into()),
    )
    .expect("second construction");
    assert_eq!(first.signed_order_ref, second.signed_order_ref);
    assert_eq!(first.signed_order_digest, second.signed_order_digest);
}

#[test]
fn standard_sign_only_plan_rejects_profile_that_can_post_or_expose_raw_order() {
    let profile = OfficialSdkStandardSignOnlyProfile {
        exposes_raw_signed_order: true,
        ..OfficialSdkStandardSignOnlyProfile::default()
    };
    let err = standard_sign_only_plan_for_order(profile, &sample_plan_limit())
        .expect_err("raw order exposure must be rejected");
    assert!(
        err.to_string()
            .contains("must not expose raw signed orders")
    );

    let profile = OfficialSdkStandardSignOnlyProfile {
        may_post_order: true,
        ..OfficialSdkStandardSignOnlyProfile::default()
    };
    let err = standard_sign_only_plan_for_order(profile, &sample_plan_limit())
        .expect_err("posting profile must be rejected");
    assert!(err.to_string().contains("post orders"));
}

#[test]
fn sign_only_request_converts_to_limit_plan() {
    let request = SignOnlyDryRunRequest {
        account_id: AccountId("acct-1".into()),
        execution_id: ExecutionId("exec-1".into()),
        plan_hash: HashValue("plan-hash-1".into()),
        token_id: "456".into(),
        side: "SELL".into(),
        size: "25".into(),
        limit_price: "0.61".into(),
    };
    let plan = request.into_plan_order();
    assert_eq!(plan.order_kind, "LIMIT");
    assert_eq!(plan.side, "SELL");
    assert_eq!(plan.time_in_force.as_deref(), Some("GTC"));
}

#[test]
fn sign_only_lifecycle_records_are_persistable_and_non_mutating() {
    let receipt = SignOnlyDryRunReceipt {
        account_id: AccountId("acct-1".into()),
        execution_id: ExecutionId("exec-1".into()),
        plan_hash: HashValue("plan-hash-1".into()),
        signed_order_ref: "sign-only:exec-1:plan-hash-1:sig-abcd".into(),
        posted: false,
    };
    let records =
        sign_only_lifecycle_records_from_receipt(&receipt).expect("sign-only lifecycle records");
    assert_eq!(records.len(), 3);
    assert!(records.iter().all(|record| record.no_remote_side_effect));
    assert_eq!(
        records.last().unwrap().state,
        SignOnlyLifecycleState::SignedDryRun
    );
    assert_eq!(
        records.last().unwrap().signed_order_ref.as_deref(),
        Some("sign-only:exec-1:plan-hash-1:sig-abcd")
    );
}

#[test]
fn sign_only_lifecycle_rejects_posted_receipt() {
    let receipt = SignOnlyDryRunReceipt {
        account_id: AccountId("acct-1".into()),
        execution_id: ExecutionId("exec-1".into()),
        plan_hash: HashValue("plan-hash-1".into()),
        signed_order_ref: "sign-only:exec-1:plan-hash-1:sig-abcd".into(),
        posted: true,
    };
    assert!(sign_only_lifecycle_records_from_receipt(&receipt).is_err());
}
