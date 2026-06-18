use super::*;

fn secret_ref(name: &str) -> SecretReference {
    SecretReference {
        provider: "kms".into(),
        reference: name.into(),
        version: "1".into(),
    }
}

#[test]
fn production_gateway_assembly_fails_closed_with_missing_bindings() {
    let decision = ProductionGatewayAssemblyRequest {
        environment: "production".into(),
        artifact_sha256: "".into(),
        evidence_manifest_sha256: "".into(),
        reviewer_registry_ref: "".into(),
        review_signature_ref: "".into(),
        clob_secret_ref: None,
        signer_secret_ref: None,
        readiness: DeploymentReadiness {
            ready: false,
            environment: "disabled".into(),
            reason: "not checked".into(),
        },
    }
    .validate();

    assert!(!decision.ready);
    assert!(
        decision
            .blockers
            .contains(&"artifact_sha256_required".to_string())
    );
    assert!(
        decision
            .blockers
            .contains(&"evidence_manifest_sha256_required".to_string())
    );
    assert!(
        decision
            .blockers
            .contains(&"reviewer_registry_ref_required".to_string())
    );
    assert!(
        decision
            .blockers
            .contains(&"review_signature_ref_required".to_string())
    );
    assert!(
        decision
            .blockers
            .contains(&"clob_secret_ref_required".to_string())
    );
    assert!(
        decision
            .blockers
            .contains(&"signer_secret_ref_required".to_string())
    );
    assert!(
        decision
            .blockers
            .contains(&"deployment_readiness_not_ready".to_string())
    );
}

#[test]
fn production_gateway_assembly_accepts_only_complete_references() {
    let decision = ProductionGatewayAssemblyRequest {
        environment: "production".into(),
        artifact_sha256: "a".repeat(64),
        evidence_manifest_sha256: "b".repeat(64),
        reviewer_registry_ref: "reviewer-registry://lei".into(),
        review_signature_ref: "external-review://signature/live-gateway-design".into(),
        clob_secret_ref: Some(secret_ref("secret://clob/l2-creds")),
        signer_secret_ref: Some(secret_ref("secret://signer/hsm")),
        readiness: DeploymentReadiness {
            ready: true,
            environment: "production".into(),
            reason: "all external gates satisfied".into(),
        },
    }
    .validate();

    assert!(decision.ready);
    assert!(decision.blockers.is_empty());
}
