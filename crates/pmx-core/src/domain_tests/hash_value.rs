use super::*;

#[test]
fn hash_value_accepts_lowercase_sha256_hex() {
    let hash = HashValue::from_sha256_hex(
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .expect("valid sha256 hex");

    assert!(hash.is_sha256_hex());
}

#[test]
fn hash_value_rejects_non_sha256_hex() {
    let err = HashValue::from_sha256_hex("not-a-hash").expect_err("invalid hash");

    assert!(matches!(err, CoreError::InvalidHashValue(_)));
}

#[test]
fn typed_ids_reject_blank_values_during_deserialization() {
    assert!(serde_json::from_str::<AccountId>(r#"" ""#).is_err());
    assert!(serde_json::from_str::<ConditionId>(r#"" ""#).is_err());
    assert!(serde_json::from_str::<TokenId>(r#"" ""#).is_err());
    assert!(serde_json::from_str::<ExecutionId>(r#"" ""#).is_err());
    assert!(serde_json::from_str::<InternalOrderId>(r#"" ""#).is_err());
    assert!(serde_json::from_str::<RemoteOrderId>(r#"" ""#).is_err());
}
