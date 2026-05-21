use serde::{Deserialize, Serialize};

use super::CoreError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConditionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TokenId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HashValue(pub String);

impl HashValue {
    pub fn from_sha256_hex(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into();
        if Self::is_sha256_hex_str(&value) {
            Ok(Self(value))
        } else {
            Err(CoreError::InvalidHashValue(value))
        }
    }

    pub fn is_sha256_hex(&self) -> bool {
        Self::is_sha256_hex_str(&self.0)
    }

    fn is_sha256_hex_str(value: &str) -> bool {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InternalOrderId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RemoteOrderId(pub String);
