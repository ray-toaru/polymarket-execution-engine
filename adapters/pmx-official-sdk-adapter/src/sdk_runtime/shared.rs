use crate::model::{L2_API_KEY_VAR, L2_API_PASSPHRASE_VAR, L2_API_SECRET_VAR};
use crate::{ENV_SDK_CALL_TIMEOUT_SECS, OfficialSdkAdapterConfig};

use anyhow::Context;
use polymarket_client_sdk_v2::auth::{
    Credentials as SdkCredentials, LocalSigner, Signer as _, Uuid,
};
use polymarket_client_sdk_v2::clob::{Client as SdkClient, Config as SdkConfig};
use polymarket_client_sdk_v2::{POLYGON, PRIVATE_KEY_VAR};
use std::str::FromStr;
use std::time::Duration;
use tokio::time;

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

#[cfg(test)]
pub(super) fn sdk_config() -> SdkConfig {
    SdkConfig::builder().use_server_time(true).build()
}
