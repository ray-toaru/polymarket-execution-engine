use super::*;

#[test]
fn plan_mapping_normalizes_limit_orders() {
    let mapping =
        official_sdk_plan_to_builder_mapping(&sample_plan_limit()).expect("limit mapping");
    assert_eq!(mapping.side, "BUY");
    assert_eq!(mapping.order_kind, "LIMIT");
    assert_eq!(mapping.time_in_force.as_deref(), Some("GTC"));
    assert_eq!(mapping.limit_price.as_deref(), Some("0.55"));
}

#[test]
fn plan_mapping_preserves_metadata_without_exposing_signed_payload() {
    let mut plan = sample_plan_limit();
    plan.builder_attribution = Some("builder-code".into());
    plan.fee_rate_bps = Some("0".into());
    plan.funder = Some("deposit-wallet".into());
    plan.signer = Some("operator-signer".into());
    plan.signature_type = Some("EOA".into());
    let mapping = official_sdk_plan_to_builder_mapping(&plan).expect("metadata mapping");
    assert_eq!(mapping.builder_attribution.as_deref(), Some("builder-code"));
    assert_eq!(mapping.funder.as_deref(), Some("deposit-wallet"));
    assert_eq!(mapping.signature_type.as_deref(), Some("EOA"));
}

#[test]
fn plan_mapping_maps_ioc_to_sdk_fak() {
    let mut plan = sample_plan_limit();
    plan.time_in_force = Some("ioc".into());
    let mapping = official_sdk_plan_to_builder_mapping(&plan).expect("ioc mapping");
    assert_eq!(mapping.time_in_force.as_deref(), Some("FAK"));
}

#[test]
fn plan_mapping_supports_fok_limit_orders() {
    let mut plan = sample_plan_limit();
    plan.time_in_force = Some("fok".into());
    let mapping = official_sdk_plan_to_builder_mapping(&plan).expect("fok mapping");
    assert_eq!(mapping.time_in_force.as_deref(), Some("FOK"));
}

#[test]
fn plan_mapping_supports_gtd_with_expiration() {
    let mut plan = sample_plan_limit();
    plan.time_in_force = Some("gtd".into());
    plan.expiration = Some("2027-01-01T00:00:00Z".into());
    let mapping = official_sdk_plan_to_builder_mapping(&plan).expect("gtd mapping");
    assert_eq!(mapping.time_in_force.as_deref(), Some("GTD"));
    assert_eq!(mapping.expiration.as_deref(), Some("2027-01-01T00:00:00Z"));
}

#[test]
fn plan_mapping_rejects_gtd_without_valid_expiration() {
    let mut plan = sample_plan_limit();
    plan.time_in_force = Some("gtd".into());
    let err = official_sdk_plan_to_builder_mapping(&plan).expect_err("gtd needs expiration");
    assert!(err.to_string().contains("GTD mapping requires expiration"));

    plan.expiration = Some("not-a-time".into());
    let err = official_sdk_plan_to_builder_mapping(&plan).expect_err("gtd needs valid expiration");
    assert!(err.to_string().contains("RFC3339"));
}

#[test]
fn plan_mapping_rejects_expiration_for_non_gtd() {
    let mut plan = sample_plan_limit();
    plan.expiration = Some("2027-01-01T00:00:00Z".into());
    let err =
        official_sdk_plan_to_builder_mapping(&plan).expect_err("expiration only valid for gtd");
    assert!(err.to_string().contains("only supported for GTD"));
}

#[test]
fn plan_mapping_requires_market_amount() {
    let mut plan = sample_plan_limit();
    plan.order_kind = "MARKET".into();
    plan.limit_price = None;
    plan.size = None;
    let err = official_sdk_plan_to_builder_mapping(&plan).expect_err("market must need amount");
    assert!(matches!(err, OfficialSdkAdapterError::InvalidInput(_)));
}

#[test]
fn plan_mapping_supports_market_amount() {
    let mut plan = sample_plan_limit();
    plan.order_kind = "market".into();
    plan.limit_price = None;
    plan.size = None;
    plan.amount = Some("12.5".into());
    plan.time_in_force = None;
    let mapping = official_sdk_plan_to_builder_mapping(&plan).expect("market mapping");
    assert_eq!(mapping.order_kind, "MARKET");
    assert_eq!(mapping.amount.as_deref(), Some("12.5"));
    assert!(mapping.time_in_force.is_none());
}

#[test]
fn plan_mapping_rejects_placeholder_token_id() {
    let mut plan = sample_plan_limit();
    plan.token_id = "replace-me".into();
    let err = official_sdk_plan_to_builder_mapping(&plan).expect_err("invalid token");
    assert!(matches!(err, OfficialSdkAdapterError::InvalidInput(_)));
}

#[test]
fn plan_mapping_rejects_invalid_limit_price_and_zero_size() {
    let mut over_one = sample_plan_limit();
    over_one.limit_price = Some("1.01".into());
    assert!(official_sdk_plan_to_builder_mapping(&over_one).is_err());

    let mut zero_size = sample_plan_limit();
    zero_size.size = Some("0".into());
    assert!(official_sdk_plan_to_builder_mapping(&zero_size).is_err());
}
