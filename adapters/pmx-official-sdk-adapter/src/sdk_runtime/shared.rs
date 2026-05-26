use crate::model::{L2_API_KEY_VAR, L2_API_PASSPHRASE_VAR, L2_API_SECRET_VAR};
use crate::{ENV_SDK_CALL_TIMEOUT_SECS, OfficialSdkAdapterConfig};

use anyhow::Context;
use polymarket_client_sdk_v2::auth::{
    Credentials as SdkCredentials, LocalSigner, Signer as _, Uuid,
};
use polymarket_client_sdk_v2::clob::types::SignatureType;
use polymarket_client_sdk_v2::clob::{Client as SdkClient, Config as SdkConfig};
use polymarket_client_sdk_v2::types::Address;
use polymarket_client_sdk_v2::{POLYGON, PRIVATE_KEY_VAR};
use std::str::FromStr;
use std::time::Duration;
use tokio::time;

use super::signature_type::{ENV_CLOB_SIGNATURE_TYPE, parse_signature_type};

const ENV_CLOB_FUNDER: &str = "PMX_CLOB_FUNDER";
const ENV_ACTIVE_ACCOUNT_PROFILE: &str = "PMX_ACTIVE_ACCOUNT_PROFILE";
const ENV_ACTIVE_ACCOUNT_ID: &str = "PMX_ACTIVE_ACCOUNT_ID";
const ENV_ACTIVE_PROFILE_REF: &str = "PMX_ACTIVE_PROFILE_REF";

pub(super) fn sdk_call_timeout() -> Duration {
    let parsed = std::env::var(ENV_SDK_CALL_TIMEOUT_SECS)
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|secs| *secs > 0);
    Duration::from_secs(parsed.unwrap_or(10))
}

pub(super) fn signer_from_env()
-> anyhow::Result<impl polymarket_client_sdk_v2::auth::Signer + Clone> {
    let private_key = std::env::var(PRIVATE_KEY_VAR)
        .with_context(|| format!("missing {PRIVATE_KEY_VAR} for official SDK signer"))?;
    let signer = LocalSigner::from_str(&private_key)
        .context("invalid POLYMARKET_PRIVATE_KEY for official SDK signer")?
        .with_chain_id(Some(POLYGON));
    Ok(signer)
}

fn sdk_credentials_from_env() -> anyhow::Result<Option<SdkCredentials>> {
    match (
        std::env::var(L2_API_KEY_VAR).ok(),
        std::env::var(L2_API_SECRET_VAR).ok(),
        std::env::var(L2_API_PASSPHRASE_VAR).ok(),
    ) {
        (Some(key), Some(secret), Some(passphrase)) => {
            let uuid = Uuid::parse_str(&key).context("invalid POLY_API_KEY UUID")?;
            Ok(Some(SdkCredentials::new(uuid, secret, passphrase)))
        }
        (None, None, None) => Ok(None),
        _ => Err(anyhow::anyhow!(
            "partial L2 credential material present; require POLY_API_KEY, POLY_API_SECRET and POLY_API_PASSPHRASE together"
        )),
    }
}

fn clob_funder_from_env() -> anyhow::Result<Option<Address>> {
    std::env::var(ENV_CLOB_FUNDER)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            Address::from_str(value.trim())
                .with_context(|| format!("invalid {ENV_CLOB_FUNDER} address"))
        })
        .transpose()
}

fn clob_signature_type_from_env(has_funder: bool) -> anyhow::Result<SignatureType> {
    let Some(raw) = std::env::var(ENV_CLOB_SIGNATURE_TYPE)
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(if has_funder {
            SignatureType::Poly1271
        } else {
            SignatureType::Eoa
        });
    };
    parse_signature_type(&raw).map_err(anyhow::Error::msg)
}

pub(super) async fn authenticated_sdk_client(
    config: &OfficialSdkAdapterConfig,
) -> anyhow::Result<
    SdkClient<
        polymarket_client_sdk_v2::auth::state::Authenticated<
            polymarket_client_sdk_v2::auth::Normal,
        >,
    >,
> {
    let signer = signer_from_env()?;
    let mut builder = SdkClient::new(
        &config.clob_host,
        SdkConfig::builder().use_server_time(true).build(),
    )
    .context("creating official SDK client")?
    .authentication_builder(&signer);
    if let Some(funder) = clob_funder_from_env()? {
        builder = builder
            .funder(funder)
            .signature_type(clob_signature_type_from_env(true)?);
    } else {
        builder = builder.signature_type(clob_signature_type_from_env(false)?);
    }
    if let Some(credentials) = sdk_credentials_from_env()? {
        builder = builder.credentials(credentials);
    }
    let timeout = sdk_call_timeout();
    let client = time::timeout(timeout, builder.authenticate())
        .await
        .map_err(|_| anyhow::anyhow!("official SDK authentication timed out after {timeout:?}"))?
        .context("official SDK authentication failed")?;
    Ok(client)
}

pub fn validate_active_profile_env_for_canary(expected_account_id: &str) -> anyhow::Result<()> {
    let required = [
        ENV_ACTIVE_ACCOUNT_PROFILE,
        ENV_ACTIVE_ACCOUNT_ID,
        ENV_ACTIVE_PROFILE_REF,
        PRIVATE_KEY_VAR,
        L2_API_KEY_VAR,
        L2_API_SECRET_VAR,
        L2_API_PASSPHRASE_VAR,
        ENV_CLOB_SIGNATURE_TYPE,
    ];
    let missing = required
        .into_iter()
        .filter(|name| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        anyhow::bail!(
            "active profile runtime env is incomplete: missing {}",
            missing.join(", ")
        );
    }
    let active_account_id = std::env::var(ENV_ACTIVE_ACCOUNT_ID)
        .expect("checked above")
        .trim()
        .to_owned();
    if active_account_id != expected_account_id {
        anyhow::bail!(
            "active profile account id mismatch: expected {expected_account_id} got {active_account_id}"
        );
    }
    let signature_type = parse_signature_type(
        &std::env::var(ENV_CLOB_SIGNATURE_TYPE)
            .expect("checked above")
            .trim()
            .to_owned(),
    )
    .map_err(anyhow::Error::msg)?;
    let funder = clob_funder_from_env()?;
    if matches!(signature_type, SignatureType::Poly1271) && funder.is_none() {
        anyhow::bail!("{ENV_CLOB_FUNDER} is required when {ENV_CLOB_SIGNATURE_TYPE}=POLY_1271");
    }
    Ok(())
}

#[cfg(all(feature = "sign-only-dry-run", test))]
pub(super) fn sdk_config() -> SdkConfig {
    SdkConfig::builder().use_server_time(true).build()
}
