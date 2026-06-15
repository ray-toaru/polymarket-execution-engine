use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::GatewayError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretReference {
    pub provider: String,
    pub reference: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlertEvent {
    pub code: String,
    pub severity: String,
    pub correlation_id: String,
    pub redacted_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentReadiness {
    pub ready: bool,
    pub environment: String,
    pub reason: String,
}

#[async_trait]
pub trait SecretProvider: Send + Sync {
    async fn resolve_reference(&self, name: &str) -> Result<SecretReference, GatewayError>;
}

#[async_trait]
pub trait AlertSink: Send + Sync {
    async fn publish(&self, event: &AlertEvent) -> Result<(), GatewayError>;
}

#[async_trait]
pub trait DeploymentReadinessProvider: Send + Sync {
    async fn readiness(&self) -> Result<DeploymentReadiness, GatewayError>;
}

#[derive(Default)]
pub struct DisabledOperationalPorts;

#[async_trait]
impl SecretProvider for DisabledOperationalPorts {
    async fn resolve_reference(&self, _name: &str) -> Result<SecretReference, GatewayError> {
        Err(GatewayError::Disabled)
    }
}

#[async_trait]
impl AlertSink for DisabledOperationalPorts {
    async fn publish(&self, _event: &AlertEvent) -> Result<(), GatewayError> {
        Err(GatewayError::Disabled)
    }
}

#[async_trait]
impl DeploymentReadinessProvider for DisabledOperationalPorts {
    async fn readiness(&self) -> Result<DeploymentReadiness, GatewayError> {
        Ok(DeploymentReadiness {
            ready: false,
            environment: "disabled".into(),
            reason: "deployment readiness provider is disabled".into(),
        })
    }
}
