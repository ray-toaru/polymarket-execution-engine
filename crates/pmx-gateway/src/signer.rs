use crate::{GatewayError, PlanOrder, Signer, SignerProvider};
use async_trait::async_trait;
use pmx_core::{AccountId, InternalOrderId, SignedOrderEnvelope};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignerBackendKind {
    Disabled,
    DeterministicTest,
    OfficialSdkLocal,
    OfficialSdkRemoteKms,
    OfficialSdkExternal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignerProviderConfig {
    pub backend: SignerBackendKind,
    pub allow_local_private_key_material: bool,
    pub require_remote_signer_in_production: bool,
}

impl Default for SignerProviderConfig {
    fn default() -> Self {
        Self {
            backend: SignerBackendKind::Disabled,
            allow_local_private_key_material: false,
            require_remote_signer_in_production: true,
        }
    }
}

#[derive(Default)]
pub struct DeterministicTestSignerProvider;

#[async_trait]
impl SignerProvider for DeterministicTestSignerProvider {
    async fn signer_for_account(
        &self,
        _account_id: &AccountId,
    ) -> Result<Arc<dyn Signer>, GatewayError> {
        Ok(Arc::new(DeterministicTestSigner))
    }
}

pub struct DeterministicTestSigner;

#[async_trait]
impl Signer for DeterministicTestSigner {
    async fn sign_order(&self, order: &PlanOrder) -> Result<SignedOrderEnvelope, GatewayError> {
        Ok(SignedOrderEnvelope {
            internal_order_id: InternalOrderId(format!("test-order-{}", order.execution_id)),
            account_id: order.account_id.clone(),
            signer_fingerprint: "deterministic-test-signer".into(),
            signed_payload_ref: "test-only-no-real-signature".into(),
        })
    }
}
