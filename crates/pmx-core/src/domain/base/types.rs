use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use super::CoreError;

macro_rules! non_empty_string_id {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
                let value = value.into();
                if value.trim().is_empty() {
                    Err(CoreError::InvalidIdentifier { field: $field })
                } else {
                    Ok(Self(value))
                }
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(de::Error::custom)
            }
        }
    };
}

non_empty_string_id!(AccountId, "account_id");
non_empty_string_id!(ConditionId, "condition_id");
non_empty_string_id!(TokenId, "token_id");

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

impl Serialize for HashValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for HashValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_sha256_hex(value).map_err(de::Error::custom)
    }
}

non_empty_string_id!(ExecutionId, "execution_id");
non_empty_string_id!(InternalOrderId, "internal_order_id");
non_empty_string_id!(RemoteOrderId, "remote_order_id");
