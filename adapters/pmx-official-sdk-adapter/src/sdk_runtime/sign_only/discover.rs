#[cfg(test)]
use super::super::shared::sdk_config;
#[cfg(test)]
use crate::OfficialSdkAdapterConfig;
#[cfg(test)]
use anyhow::Context;
#[cfg(test)]
use polymarket_client_sdk_v2::clob::Client as SdkClient;
#[cfg(test)]
use tokio::time;

#[cfg(test)]
pub(crate) async fn discover_active_token_id(
    config: &OfficialSdkAdapterConfig,
) -> anyhow::Result<String> {
    let client = SdkClient::new(&config.clob_host, sdk_config())
        .context("creating public official SDK client")?;
    let timeout = super::super::shared::sdk_call_timeout();
    let markets = time::timeout(timeout, client.simplified_markets(None))
        .await
        .map_err(|_| {
            anyhow::anyhow!("official SDK simplified_markets() timed out after {timeout:?}")
        })?
        .context("official SDK simplified_markets() failed")?;

    let token_id = markets
        .data
        .iter()
        .find(|market| {
            market.active && !market.closed && !market.archived && market.accepting_orders
        })
        .and_then(|market| market.tokens.first())
        .map(|token| token.token_id.to_string())
        .or_else(|| {
            markets.data.iter().find_map(|market| {
                market
                    .tokens
                    .first()
                    .map(|token| token.token_id.to_string())
            })
        })
        .ok_or_else(|| {
            anyhow::anyhow!("no simplified market token_id available for sign-only dry-run")
        })?;

    Ok(token_id)
}
