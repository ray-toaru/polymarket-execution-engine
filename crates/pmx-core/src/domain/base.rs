use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    OrderEventKind, OrderLifecycleState, SignOnlyLifecycleEventKind, SignOnlyLifecycleState,
};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CoreError {
    #[error("exactly one quantity bound is required")]
    QuantityBoundCardinality,
    #[error("decimal string is invalid: {0}")]
    InvalidDecimal(String),
    #[error("quantity must be a positive canonical decimal: {0}")]
    InvalidQuantity(String),
    #[error("limit_price must be a canonical decimal in (0, 1]: {0}")]
    InvalidLimitPrice(String),
    #[error("unsupported quantity bound for side: {0}")]
    UnsupportedQuantityBound(String),
    #[error("canonical JSON serialization failed: {0}")]
    CanonicalJson(String),
    #[error("invalid state transition: {from:?} -> {event:?}")]
    InvalidTransition {
        from: OrderLifecycleState,
        event: OrderEventKind,
    },
    #[error("invalid sign-only transition: {from:?} -> {event:?}")]
    InvalidSignOnlyTransition {
        from: SignOnlyLifecycleState,
        event: SignOnlyLifecycleEventKind,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConditionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TokenId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HashValue(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InternalOrderId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RemoteOrderId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecimalString(pub String);

impl DecimalString {
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_decimal_string(&self.0)
    }

    pub fn validate_positive(&self) -> Result<(), CoreError> {
        validate_positive_decimal_string(&self.0)
    }

    pub fn validate_limit_price(&self) -> Result<(), CoreError> {
        validate_limit_price_decimal_string(&self.0)
    }
}

pub fn validate_decimal_string(raw: &str) -> Result<(), CoreError> {
    if raw.is_empty() || raw.trim() != raw {
        return Err(CoreError::InvalidDecimal(raw.to_string()));
    }
    if raw.contains('e') || raw.contains('E') || raw.contains('+') || raw.contains('-') {
        return Err(CoreError::InvalidDecimal(raw.to_string()));
    }
    let parts: Vec<&str> = raw.split('.').collect();
    if parts.len() > 2 || parts[0].is_empty() {
        return Err(CoreError::InvalidDecimal(raw.to_string()));
    }
    if !parts[0].chars().all(|c| c.is_ascii_digit()) {
        return Err(CoreError::InvalidDecimal(raw.to_string()));
    }
    if parts[0].len() > 1 && parts[0].starts_with('0') {
        return Err(CoreError::InvalidDecimal(raw.to_string()));
    }
    if parts.len() == 2 && (parts[1].is_empty() || !parts[1].chars().all(|c| c.is_ascii_digit())) {
        return Err(CoreError::InvalidDecimal(raw.to_string()));
    }
    Ok(())
}

pub fn validate_positive_decimal_string(raw: &str) -> Result<(), CoreError> {
    validate_decimal_string(raw)?;
    if is_zero_decimal(raw) {
        return Err(CoreError::InvalidQuantity(raw.to_string()));
    }
    Ok(())
}

pub fn validate_limit_price_decimal_string(raw: &str) -> Result<(), CoreError> {
    validate_decimal_string(raw).map_err(|_| CoreError::InvalidLimitPrice(raw.to_string()))?;
    if is_zero_decimal(raw) || !decimal_leq_one(raw) {
        return Err(CoreError::InvalidLimitPrice(raw.to_string()));
    }
    Ok(())
}

fn is_zero_decimal(raw: &str) -> bool {
    raw.chars().filter(|c| *c != '.').all(|c| c == '0')
}

fn decimal_leq_one(raw: &str) -> bool {
    let mut parts = raw.split('.');
    let int = parts.next().unwrap_or("");
    let frac = parts.next().unwrap_or("");
    match int {
        "0" => true,
        "1" => frac.chars().all(|c| c == '0'),
        _ => false,
    }
}

fn sort_json_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut sorted = Map::new();
            for (key, value) in entries {
                sorted.insert(key, sort_json_value(value));
            }
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(sort_json_value).collect()),
        other => other,
    }
}

pub fn canonical_json_string<T: Serialize>(value: &T) -> Result<String, CoreError> {
    let json_value =
        serde_json::to_value(value).map_err(|err| CoreError::CanonicalJson(err.to_string()))?;
    serde_json::to_string(&sort_json_value(json_value))
        .map_err(|err| CoreError::CanonicalJson(err.to_string()))
}

pub fn canonical_json_sha256<T: Serialize>(value: &T) -> Result<HashValue, CoreError> {
    let canonical = canonical_json_string(value)?;
    let digest = Sha256::digest(canonical.as_bytes());
    Ok(HashValue(to_lower_hex(&digest)))
}

fn to_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
