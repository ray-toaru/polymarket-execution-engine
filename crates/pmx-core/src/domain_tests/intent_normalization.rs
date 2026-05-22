use super::*;

#[test]
fn decimal_rejects_scientific_padding_and_trailing_dot() {
    for bad in ["", " 1", "1 ", "1e-3", "+1", "-1", ".5", "1.", "00.1"] {
        assert!(
            validate_decimal_string(bad).is_err(),
            "{bad} should be invalid"
        );
    }
    assert!(validate_decimal_string("0.5").is_ok());
}

#[test]
fn decimal_multiplication_preserves_canonical_fixed_point() {
    assert_eq!(
        DecimalString("0.5".into())
            .checked_mul(&DecimalString("5".into()))
            .unwrap(),
        DecimalString("2.5".into())
    );
    assert_eq!(
        DecimalString("0.01".into())
            .checked_mul(&DecimalString("5".into()))
            .unwrap(),
        DecimalString("0.05".into())
    );
    assert_eq!(
        DecimalString("1.20".into())
            .checked_mul(&DecimalString("3.00".into()))
            .unwrap(),
        DecimalString("3.6".into())
    );
}

#[test]
fn limit_price_is_executor_authoritative() {
    for bad in ["0", "0.0", "1.01", "2", "1.0001"] {
        let mut intent = base_intent(
            Side::Buy,
            QuantityIntent {
                max_notional: Some(DecimalString("10".into())),
                max_shares: None,
            },
        );
        intent.limit_price = DecimalString(bad.into());
        assert!(matches!(
            normalize_intent(intent),
            Err(CoreError::InvalidLimitPrice(_))
        ));
    }
    let mut intent = base_intent(
        Side::Buy,
        QuantityIntent {
            max_notional: Some(DecimalString("10".into())),
            max_shares: None,
        },
    );
    intent.limit_price = DecimalString("1".into());
    assert!(normalize_intent(intent).is_ok());
}

#[test]
fn quantity_must_be_positive() {
    let intent = base_intent(
        Side::Buy,
        QuantityIntent {
            max_notional: Some(DecimalString("0".into())),
            max_shares: None,
        },
    );
    assert!(matches!(
        normalize_intent(intent),
        Err(CoreError::InvalidQuantity(_))
    ));
}

#[test]
fn quantity_requires_exactly_one_bound() {
    let intent = base_intent(
        Side::Buy,
        QuantityIntent {
            max_notional: None,
            max_shares: None,
        },
    );
    assert_eq!(
        normalize_intent(intent).unwrap_err(),
        CoreError::QuantityBoundCardinality
    );
}

#[test]
fn buy_notional_canonicalizes_to_quote_bound() {
    let n = normalize_intent(base_intent(
        Side::Buy,
        QuantityIntent {
            max_notional: Some(DecimalString("10".into())),
            max_shares: None,
        },
    ))
    .unwrap();
    assert!(matches!(
        n.quantity_bound,
        QuantityBound::WorstCaseQuoteNotional(_)
    ));
}

#[test]
fn sell_shares_canonicalizes_to_base_bound() {
    let n = normalize_intent(base_intent(
        Side::Sell,
        QuantityIntent {
            max_notional: None,
            max_shares: Some(DecimalString("7".into())),
        },
    ))
    .unwrap();
    assert!(matches!(
        n.quantity_bound,
        QuantityBound::WorstCaseBaseShares(_)
    ));
}

#[test]
fn buy_shares_canonicalizes_to_base_bound() {
    let n = normalize_intent(base_intent(
        Side::Buy,
        QuantityIntent {
            max_notional: None,
            max_shares: Some(DecimalString("7".into())),
        },
    ))
    .unwrap();
    assert!(matches!(
        n.quantity_bound,
        QuantityBound::WorstCaseBaseShares(_)
    ));
}

#[test]
fn sell_notional_remains_unsupported_without_base_conversion_rule() {
    let n = normalize_intent(base_intent(
        Side::Sell,
        QuantityIntent {
            max_notional: Some(DecimalString("7".into())),
            max_shares: None,
        },
    ))
    .unwrap();
    assert!(matches!(n.quantity_bound, QuantityBound::Unsupported(_)));
}

#[test]
fn canonical_json_hash_is_key_order_independent() {
    #[derive(Serialize)]
    struct Left {
        b: u8,
        a: u8,
    }
    #[derive(Serialize)]
    struct Right {
        a: u8,
        b: u8,
    }

    let left = canonical_json_sha256(&Left { b: 2, a: 1 }).unwrap();
    let right = canonical_json_sha256(&Right { a: 1, b: 2 }).unwrap();
    assert_eq!(left, right);
    assert_eq!(left.0.len(), 64);
}

#[test]
fn normalized_intent_hash_is_content_derived() {
    let first = normalize_intent(base_intent(
        Side::Buy,
        QuantityIntent {
            max_notional: Some(DecimalString("10".into())),
            max_shares: None,
        },
    ))
    .unwrap();
    let second = normalize_intent(base_intent(
        Side::Buy,
        QuantityIntent {
            max_notional: Some(DecimalString("10".into())),
            max_shares: None,
        },
    ))
    .unwrap();
    assert_eq!(first.intent_hash, second.intent_hash);
    assert!(first.normalized_intent_id.starts_with("norm-"));
}
