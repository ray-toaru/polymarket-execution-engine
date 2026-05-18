use crate::{
    AdapterCredentialSnapshot, ENV_ALLOW_LIVE_SUBMIT, ENV_ALLOW_SIGN_ONLY_DRY_RUN,
    ENV_RUN_AUTHENTICATED_SMOKE, ENV_RUN_SIGN_ONLY_DRY_RUN, OfficialSdkAdapterConfig,
    OfficialSdkAdapterError, env_flag,
};

pub fn validate_read_only_smoke_environment(
    _credentials: &AdapterCredentialSnapshot,
) -> Result<(), OfficialSdkAdapterError> {
    Ok(())
}

pub fn validate_authenticated_non_trading_smoke(
    config: &OfficialSdkAdapterConfig,
    credentials: &AdapterCredentialSnapshot,
) -> Result<(), OfficialSdkAdapterError> {
    if !config.allow_authenticated_non_trading_smoke || !env_flag(ENV_RUN_AUTHENTICATED_SMOKE) {
        return Err(OfficialSdkAdapterError::SafetyGate(format!(
            "set {ENV_RUN_AUTHENTICATED_SMOKE}=1 and config.allow_authenticated_non_trading_smoke=true"
        )));
    }
    if !credentials.has_authenticated_material() {
        return Err(OfficialSdkAdapterError::MissingCredential(
            "authenticated non-trading smoke needs L1 or complete L2 credentials".into(),
        ));
    }
    Ok(())
}

pub fn validate_sign_only_dry_run(
    config: &OfficialSdkAdapterConfig,
    credentials: &AdapterCredentialSnapshot,
) -> Result<(), OfficialSdkAdapterError> {
    if config.allow_live_submit || env_flag(ENV_ALLOW_LIVE_SUBMIT) || cfg!(feature = "live-submit")
    {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "sign-only dry-run must not run in a live-submit-enabled process".into(),
        ));
    }
    if !config.allow_sign_only_dry_run
        || !env_flag(ENV_RUN_SIGN_ONLY_DRY_RUN)
        || !env_flag(ENV_ALLOW_SIGN_ONLY_DRY_RUN)
    {
        return Err(OfficialSdkAdapterError::SafetyGate(format!(
            "set {ENV_RUN_SIGN_ONLY_DRY_RUN}=1, {ENV_ALLOW_SIGN_ONLY_DRY_RUN}=1 and config.allow_sign_only_dry_run=true"
        )));
    }
    if !credentials.has_l1_private_key {
        return Err(OfficialSdkAdapterError::MissingCredential(
            "sign-only dry-run needs an L1 signer, but must not post the order".into(),
        ));
    }
    Ok(())
}
