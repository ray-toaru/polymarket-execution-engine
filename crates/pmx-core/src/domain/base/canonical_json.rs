use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::{CoreError, HashValue};

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
