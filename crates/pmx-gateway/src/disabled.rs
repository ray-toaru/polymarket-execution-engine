use crate::{
    ClobGateway, GatewayError, PostOrderAck, RemoteOrder, RemoteReconcileReadReport,
    RemoteReconcileReadRequest, Signer, SignerProvider,
};
use async_trait::async_trait;
use pmx_core::{AccountId, CancelState, RemoteOrderId, SignedOrderEnvelope};
use std::sync::Arc;

#[derive(Default)]
pub struct DisabledSignerProvider;

#[async_trait]
impl SignerProvider for DisabledSignerProvider {
    async fn signer_for_account(
        &self,
        _account_id: &AccountId,
    ) -> Result<Arc<dyn Signer>, GatewayError> {
        Err(GatewayError::SigningUnavailable)
    }
}

pub struct DisabledSigner;

#[async_trait]
impl Signer for DisabledSigner {
    async fn sign_order(
        &self,
        _order: &crate::PlanOrder,
    ) -> Result<SignedOrderEnvelope, GatewayError> {
        Err(GatewayError::SigningUnavailable)
    }
}

pub struct DisabledGateway;

#[async_trait]
impl ClobGateway for DisabledGateway {
    async fn post_order(&self, _order: &SignedOrderEnvelope) -> Result<PostOrderAck, GatewayError> {
        Err(GatewayError::Disabled)
    }

    async fn cancel_order(
        &self,
        _account_id: &AccountId,
        _remote_order_id: &RemoteOrderId,
    ) -> Result<CancelState, GatewayError> {
        Err(GatewayError::Disabled)
    }

    async fn get_order(
        &self,
        _account_id: &AccountId,
        _remote_order_id: &RemoteOrderId,
    ) -> Result<Option<RemoteOrder>, GatewayError> {
        Err(GatewayError::Disabled)
    }

    async fn get_open_orders(
        &self,
        _account_id: &AccountId,
    ) -> Result<Vec<RemoteOrder>, GatewayError> {
        Err(GatewayError::Disabled)
    }
}

#[async_trait]
impl crate::RemoteReconcileReader for DisabledGateway {
    async fn read_remote_order_observations(
        &self,
        _request: &RemoteReconcileReadRequest,
    ) -> Result<RemoteReconcileReadReport, GatewayError> {
        Err(GatewayError::Disabled)
    }
}

#[async_trait]
impl crate::MarketDataReader for DisabledGateway {
    async fn read_market_book(
        &self,
        _condition_id: &pmx_core::ConditionId,
        _token_id: &pmx_core::TokenId,
    ) -> Result<pmx_core::MarketBookSnapshot, GatewayError> {
        Err(GatewayError::Disabled)
    }
}
