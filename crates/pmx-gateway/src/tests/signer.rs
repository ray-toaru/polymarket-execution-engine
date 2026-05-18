use super::*;

#[tokio::test]
async fn disabled_signer_provider_refuses_to_materialize_signer() {
    let provider = DisabledSignerProvider;
    let result = provider
        .signer_for_account(&pmx_core::AccountId("acct-disabled".into()))
        .await;
    match result {
        Err(err) => assert_eq!(err, GatewayError::SigningUnavailable),
        Ok(_) => panic!("disabled provider must fail"),
    }
}

#[test]
fn signer_provider_defaults_are_production_conservative() {
    let cfg = SignerProviderConfig::default();
    assert_eq!(cfg.backend, SignerBackendKind::Disabled);
    assert!(!cfg.allow_local_private_key_material);
    assert!(cfg.require_remote_signer_in_production);
}
