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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionGatewayAssemblyRequest {
    pub environment: String,
    pub artifact_sha256: String,
    pub evidence_manifest_sha256: String,
    pub reviewer_registry_ref: String,
    pub review_signature_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clob_secret_ref: Option<SecretReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signer_secret_ref: Option<SecretReference>,
    pub readiness: DeploymentReadiness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionGatewayAssemblyDecision {
    pub ready: bool,
    pub blockers: Vec<String>,
}

impl ProductionGatewayAssemblyRequest {
    pub fn validate(&self) -> ProductionGatewayAssemblyDecision {
        let mut blockers = Vec::new();
        if self.environment != "production" {
            blockers.push("environment_must_be_production".to_string());
        }
        require_sha256(
            &self.artifact_sha256,
            "artifact_sha256_required",
            "artifact_sha256_invalid",
            &mut blockers,
        );
        require_sha256(
            &self.evidence_manifest_sha256,
            "evidence_manifest_sha256_required",
            "evidence_manifest_sha256_invalid",
            &mut blockers,
        );
        require_non_empty_ref(
            &self.reviewer_registry_ref,
            "reviewer_registry_ref_required",
            &mut blockers,
        );
        require_non_empty_ref(
            &self.review_signature_ref,
            "review_signature_ref_required",
            &mut blockers,
        );
        require_secret_ref(&self.clob_secret_ref, "clob_secret_ref", &mut blockers);
        require_secret_ref(&self.signer_secret_ref, "signer_secret_ref", &mut blockers);
        if !self.readiness.ready || self.readiness.environment != "production" {
            blockers.push("deployment_readiness_not_ready".to_string());
        }
        ProductionGatewayAssemblyDecision {
            ready: blockers.is_empty(),
            blockers,
        }
    }
}

fn require_non_empty_ref(value: &str, blocker: &str, blockers: &mut Vec<String>) {
    if value.trim().is_empty() {
        blockers.push(blocker.to_string());
    }
}

fn require_sha256(value: &str, missing: &str, invalid: &str, blockers: &mut Vec<String>) {
    let value = value.trim();
    if value.is_empty() {
        blockers.push(missing.to_string());
    } else if value.len() != 64 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
        blockers.push(invalid.to_string());
    }
}

fn require_secret_ref(value: &Option<SecretReference>, name: &str, blockers: &mut Vec<String>) {
    let Some(value) = value else {
        blockers.push(format!("{name}_required"));
        return;
    };
    if value.provider.trim().is_empty()
        || value.reference.trim().is_empty()
        || value.version.trim().is_empty()
    {
        blockers.push(format!("{name}_invalid"));
    }
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
