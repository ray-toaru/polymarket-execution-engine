use crate::{
    OfficialSdkAdapterConfig, OfficialSdkAdapterError, gateway_error_from_normalized_sdk_error,
    normalize_sdk_error, redact_sensitive_text,
};

use super::shared::{authenticated_sdk_client, sdk_call_timeout, signer_from_env};

use anyhow::Context;
use async_trait::async_trait;
use pmx_core::{AccountId, CancelState, InternalOrderId, RemoteOrderId, SignedOrderEnvelope};
use pmx_gateway::{ClobGateway, GatewayError, PlanOrder, PostOrderAck, RemoteOrder, Signer};
use polymarket_client_sdk_v2::auth::{Normal, state::Authenticated};
use polymarket_client_sdk_v2::clob::Client as SdkClient;
use polymarket_client_sdk_v2::clob::types::request::OrdersRequest;
use polymarket_client_sdk_v2::clob::types::{OrderType as SdkOrderType, Side as SdkSide};
use polymarket_client_sdk_v2::types::{Decimal as SdkDecimal, U256 as SdkU256};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tokio::time;

type AuthenticatedClient = SdkClient<Authenticated<Normal>>;
type SignedOrder = polymarket_client_sdk_v2::clob::types::SignedOrder;

#[derive(Clone)]
pub struct OfficialSdkSignerProvider {
    client: AuthenticatedClient,
    signed_orders: Arc<Mutex<HashMap<String, SignedOrder>>>,
}

#[derive(Clone)]
pub struct OfficialSdkGateway {
    client: AuthenticatedClient,
    signed_orders: Arc<Mutex<HashMap<String, SignedOrder>>>,
}

pub async fn official_sdk_gateway_pair(
    config: &OfficialSdkAdapterConfig,
) -> anyhow::Result<(OfficialSdkSignerProvider, OfficialSdkGateway)> {
    if !config.allow_live_submit {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "official SDK gateway requires allow_live_submit=true".into(),
        )
        .into());
    }
    let client = authenticated_sdk_client(config).await?;
    let signed_orders = Arc::new(Mutex::new(HashMap::new()));
    Ok((
        OfficialSdkSignerProvider {
            client: client.clone(),
            signed_orders: signed_orders.clone(),
        },
        OfficialSdkGateway {
            client,
            signed_orders,
        },
    ))
}

#[async_trait]
impl pmx_gateway::SignerProvider for OfficialSdkSignerProvider {
    async fn signer_for_account(
        &self,
        _account_id: &AccountId,
    ) -> Result<Arc<dyn Signer>, GatewayError> {
        Ok(Arc::new(OfficialSdkSigner {
            client: self.client.clone(),
            signed_orders: self.signed_orders.clone(),
        }))
    }
}

struct OfficialSdkSigner {
    client: AuthenticatedClient,
    signed_orders: Arc<Mutex<HashMap<String, SignedOrder>>>,
}

#[async_trait]
impl Signer for OfficialSdkSigner {
    async fn sign_order(&self, order: &PlanOrder) -> Result<SignedOrderEnvelope, GatewayError> {
        let token_id = SdkU256::from_str(&order.token_id.0)
            .map_err(|err| GatewayError::RemoteRejected(format!("invalid token_id: {err}")))?;
        let price = SdkDecimal::from_str(&order.limit_price)
            .map_err(|err| GatewayError::RemoteRejected(format!("invalid limit_price: {err}")))?;
        let size = SdkDecimal::from_str(&order.size)
            .map_err(|err| GatewayError::RemoteRejected(format!("invalid size: {err}")))?;
        let side = sdk_side(&order.side)?;
        let order_type = sdk_order_type(&order.time_in_force)?;
        let timeout = sdk_call_timeout();
        let signer = signer_from_env().map_err(|_| GatewayError::SigningUnavailable)?;
        let signable = time::timeout(
            timeout,
            self.client
                .limit_order()
                .token_id(token_id)
                .price(price)
                .size(size)
                .side(side)
                .order_type(order_type)
                .build(),
        )
        .await
        .map_err(|_| GatewayError::RemoteUnknown(format!("SDK build timed out after {timeout:?}")))?
        .map_err(|err| gateway_error_from_normalized_sdk_error(&normalize_sdk_error(&err)))?;
        let signed = time::timeout(timeout, self.client.sign(&signer, signable))
            .await
            .map_err(|_| {
                GatewayError::RemoteUnknown(format!("SDK sign timed out after {timeout:?}"))
            })?
            .map_err(|err| gateway_error_from_normalized_sdk_error(&normalize_sdk_error(&err)))?;
        let signed_payload_ref = signed_order_ref(&order.execution_id, &signed)
            .map_err(|err| GatewayError::RemoteUnknown(redact_sensitive_text(&err.to_string())))?;
        self.signed_orders
            .lock()
            .map_err(|_| GatewayError::RemoteUnknown("signed order cache poisoned".into()))?
            .insert(signed_payload_ref.clone(), signed);
        Ok(SignedOrderEnvelope {
            internal_order_id: InternalOrderId(format!("sdk-order-{}", order.execution_id)),
            account_id: order.account_id.clone(),
            signer_fingerprint: "official-sdk-local-signer".into(),
            signed_payload_ref,
        })
    }
}

#[async_trait]
impl ClobGateway for OfficialSdkGateway {
    async fn post_order(&self, order: &SignedOrderEnvelope) -> Result<PostOrderAck, GatewayError> {
        let signed = self
            .signed_orders
            .lock()
            .map_err(|_| GatewayError::RemoteUnknown("signed order cache poisoned".into()))?
            .remove(&order.signed_payload_ref)
            .ok_or_else(|| {
                GatewayError::RemoteRejected(
                    "signed payload ref was not produced by this gateway instance".into(),
                )
            })?;
        let timeout = sdk_call_timeout();
        let response = time::timeout(timeout, self.client.post_order(signed))
            .await
            .map_err(|_| {
                GatewayError::RemoteUnknown(format!("SDK post_order timed out after {timeout:?}"))
            })?
            .map_err(|err| gateway_error_from_normalized_sdk_error(&normalize_sdk_error(&err)))?;
        if !response.success {
            return Err(GatewayError::RemoteRejected(redact_sensitive_text(
                &response
                    .error_msg
                    .unwrap_or_else(|| "SDK rejected order".into()),
            )));
        }
        Ok(PostOrderAck {
            remote_order_id: RemoteOrderId(response.order_id),
            accepted_at_ms: chrono::Utc::now().timestamp_millis(),
        })
    }

    async fn cancel_order(
        &self,
        account_id: &AccountId,
        remote_order_id: &RemoteOrderId,
    ) -> Result<CancelState, GatewayError> {
        let timeout = sdk_call_timeout();
        let response = time::timeout(timeout, self.client.cancel_order(&remote_order_id.0))
            .await
            .map_err(|_| {
                GatewayError::RemoteUnknown(format!("SDK cancel_order timed out after {timeout:?}"))
            })?
            .map_err(|err| gateway_error_from_normalized_sdk_error(&normalize_sdk_error(&err)))?;
        if response.canceled.iter().any(|id| id == &remote_order_id.0) {
            Ok(CancelState::RemoteAccepted)
        } else {
            let reason = response
                .not_canceled
                .get(&remote_order_id.0)
                .cloned()
                .unwrap_or_else(|| format!("account={} cancel not confirmed", account_id.0));
            Err(GatewayError::RemoteUnknown(redact_sensitive_text(&reason)))
        }
    }

    async fn get_order(
        &self,
        account_id: &AccountId,
        remote_order_id: &RemoteOrderId,
    ) -> Result<Option<RemoteOrder>, GatewayError> {
        let request = OrdersRequest::builder()
            .order_id(remote_order_id.0.clone())
            .build();
        let timeout = sdk_call_timeout();
        let page = time::timeout(timeout, self.client.orders(&request, None))
            .await
            .map_err(|_| {
                GatewayError::RemoteUnknown(format!("SDK orders(id) timed out after {timeout:?}"))
            })?
            .map_err(|err| gateway_error_from_normalized_sdk_error(&normalize_sdk_error(&err)))?;
        Ok(page.data.into_iter().next().map(|order| RemoteOrder {
            remote_order_id: RemoteOrderId(order.id),
            account_id: account_id.clone(),
            state: format!("{:?}", order.status),
        }))
    }

    async fn get_open_orders(
        &self,
        account_id: &AccountId,
    ) -> Result<Vec<RemoteOrder>, GatewayError> {
        let request = OrdersRequest::builder().build();
        let timeout = sdk_call_timeout();
        let page = time::timeout(timeout, self.client.orders(&request, None))
            .await
            .map_err(|_| {
                GatewayError::RemoteUnknown(format!("SDK orders() timed out after {timeout:?}"))
            })?
            .map_err(|err| gateway_error_from_normalized_sdk_error(&normalize_sdk_error(&err)))?;
        Ok(page
            .data
            .into_iter()
            .map(|order| RemoteOrder {
                remote_order_id: RemoteOrderId(order.id),
                account_id: account_id.clone(),
                state: format!("{:?}", order.status),
            })
            .collect())
    }
}

fn sdk_side(raw: &str) -> Result<SdkSide, GatewayError> {
    match raw {
        "BUY" | "Buy" => Ok(SdkSide::Buy),
        "SELL" | "Sell" => Ok(SdkSide::Sell),
        other => Err(GatewayError::RemoteRejected(format!(
            "unsupported SDK side: {other}"
        ))),
    }
}

fn sdk_order_type(raw: &str) -> Result<SdkOrderType, GatewayError> {
    match raw {
        "FOK" | "Fok" => Ok(SdkOrderType::FOK),
        "FAK" | "Fak" | "IOC" | "Ioc" => Ok(SdkOrderType::FAK),
        "GTC" | "Gtc" => Ok(SdkOrderType::GTC),
        "GTD" | "Gtd" => Ok(SdkOrderType::GTD),
        other => Err(GatewayError::RemoteRejected(format!(
            "unsupported SDK order type: {other}"
        ))),
    }
}

fn signed_order_ref(execution_id: &str, signed: &SignedOrder) -> anyhow::Result<String> {
    let encoded = serde_json::to_vec(signed).context("serialize signed SDK order for digest")?;
    let digest = Sha256::digest(&encoded);
    Ok(format!("official-sdk-signed:{execution_id}:{digest:x}"))
}
