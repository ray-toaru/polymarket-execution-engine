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
