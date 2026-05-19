use serde::{Deserialize, Serialize};

use super::CoreError;

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
