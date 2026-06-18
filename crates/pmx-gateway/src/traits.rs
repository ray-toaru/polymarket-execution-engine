use crate::{
    GatewayError, PlanOrder, PostOrderAck, RemoteOrder, RemoteReconcileReadReport,
    RemoteReconcileReadRequest,
};
use async_trait::async_trait;
use pmx_core::{
    AccountId, CancelState, ConditionId, MarketBookSnapshot, RemoteOrderId, SignedOrderEnvelope,
    TokenId,
};
use std::sync::Arc;

#[async_trait]
pub trait Signer: Send + Sync {
    async fn sign_order(&self, order: &PlanOrder) -> Result<SignedOrderEnvelope, GatewayError>;
}

#[async_trait]
pub trait ClobGateway: Send + Sync {
    async fn post_order(&self, order: &SignedOrderEnvelope) -> Result<PostOrderAck, GatewayError>;
    async fn discard_signed_order(&self, _order: &SignedOrderEnvelope) -> Result<(), GatewayError> {
        Ok(())
    }
    async fn cancel_order(
        &self,
        account_id: &AccountId,
        remote_order_id: &RemoteOrderId,
    ) -> Result<CancelState, GatewayError>;
    async fn get_order(
        &self,
        account_id: &AccountId,
        remote_order_id: &RemoteOrderId,
    ) -> Result<Option<RemoteOrder>, GatewayError>;
    async fn get_open_orders(
        &self,
        account_id: &AccountId,
    ) -> Result<Vec<RemoteOrder>, GatewayError>;
}

#[async_trait]
pub trait RemoteReconcileReader: Send + Sync {
    async fn read_remote_order_observations(
        &self,
        request: &RemoteReconcileReadRequest,
    ) -> Result<RemoteReconcileReadReport, GatewayError>;
}

#[async_trait]
pub trait MarketDataReader: Send + Sync {
    async fn read_market_book(
        &self,
        condition_id: &ConditionId,
        token_id: &TokenId,
    ) -> Result<MarketBookSnapshot, GatewayError>;
}

#[async_trait]
pub trait SignerProvider: Send + Sync {
    async fn signer_for_account(
        &self,
        account_id: &AccountId,
    ) -> Result<Arc<dyn Signer>, GatewayError>;
}
