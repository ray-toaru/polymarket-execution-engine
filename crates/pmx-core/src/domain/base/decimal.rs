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

    pub fn checked_mul(&self, rhs: &DecimalString) -> Result<DecimalString, CoreError> {
        checked_mul_decimal_strings(&self.0, &rhs.0)
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

fn checked_mul_decimal_strings(left: &str, right: &str) -> Result<DecimalString, CoreError> {
    validate_decimal_string(left)?;
    validate_decimal_string(right)?;
    let (left_digits, left_scale) = decimal_digits_and_scale(left);
    let (right_digits, right_scale) = decimal_digits_and_scale(right);
    let product = mul_decimal_digits(&left_digits, &right_digits);
    Ok(DecimalString(format_decimal_digits(
        product,
        left_scale + right_scale,
    )))
}

fn decimal_digits_and_scale(raw: &str) -> (Vec<u8>, usize) {
    let mut scale = 0;
    let digits = if let Some((int, frac)) = raw.split_once('.') {
        scale = frac.len();
        format!("{int}{frac}")
    } else {
        raw.to_owned()
    };
    (
        digits.bytes().map(|byte| byte - b'0').collect::<Vec<u8>>(),
        scale,
    )
}

fn mul_decimal_digits(left: &[u8], right: &[u8]) -> Vec<u8> {
    let mut product = vec![0u32; left.len() + right.len()];
    for (i, &l) in left.iter().rev().enumerate() {
        for (j, &r) in right.iter().rev().enumerate() {
            product[i + j] += u32::from(l) * u32::from(r);
        }
    }
    for idx in 0..product.len() - 1 {
        let carry = product[idx] / 10;
        product[idx] %= 10;
        product[idx + 1] += carry;
    }
    while product.len() > 1 && product.last() == Some(&0) {
        product.pop();
    }
    product
        .into_iter()
        .rev()
        .map(|digit| digit as u8)
        .collect::<Vec<u8>>()
}

fn format_decimal_digits(mut digits: Vec<u8>, scale: usize) -> String {
    while digits.len() <= scale {
        digits.insert(0, 0);
    }
    if scale == 0 {
        return trim_leading_zeros(digits);
    }
    let split_at = digits.len() - scale;
    let integer = trim_leading_zeros(digits[..split_at].to_vec());
    let mut fraction = digits[split_at..]
        .iter()
        .map(|digit| char::from(b'0' + *digit))
        .collect::<String>();
    while fraction.ends_with('0') {
        fraction.pop();
    }
    if fraction.is_empty() {
        integer
    } else {
        format!("{integer}.{fraction}")
    }
}

fn trim_leading_zeros(digits: Vec<u8>) -> String {
    let mut rendered = digits
        .into_iter()
        .skip_while(|digit| *digit == 0)
        .map(|digit| char::from(b'0' + digit))
        .collect::<String>();
    if rendered.is_empty() {
        rendered.push('0');
    }
    rendered
}
