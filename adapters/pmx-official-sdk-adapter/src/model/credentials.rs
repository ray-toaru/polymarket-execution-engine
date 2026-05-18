use serde::{Deserialize, Serialize};

use super::{L2_API_KEY_VAR, L2_API_PASSPHRASE_VAR, L2_API_SECRET_VAR, PRIVATE_KEY_VAR_NAME};
use crate::model::env_present;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterCredentialSnapshot {
    pub has_l1_private_key: bool,
    pub has_l2_api_key: bool,
    pub has_l2_api_secret: bool,
    pub has_l2_passphrase: bool,
}

impl AdapterCredentialSnapshot {
    pub fn from_env() -> Self {
        Self {
            has_l1_private_key: env_present(PRIVATE_KEY_VAR_NAME),
            has_l2_api_key: env_present(L2_API_KEY_VAR),
            has_l2_api_secret: env_present(L2_API_SECRET_VAR),
            has_l2_passphrase: env_present(L2_API_PASSPHRASE_VAR),
        }
    }

    pub fn no_sensitive_material(&self) -> bool {
        !self.has_l1_private_key
            && !self.has_l2_api_key
            && !self.has_l2_api_secret
            && !self.has_l2_passphrase
    }

    pub fn has_authenticated_material(&self) -> bool {
        self.has_l1_private_key
            || (self.has_l2_api_key && self.has_l2_api_secret && self.has_l2_passphrase)
    }
}
