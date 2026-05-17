use async_trait::async_trait;
use pmx_core::{
    AccountId, CancelState, InternalOrderId, RemoteOrderId, SignedOrderEnvelope, TokenId,
};
#[cfg(test)]
use pmx_core::{OrderEventKind, OrderLifecycleState, transition_order_state};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GatewayError {
    #[error("remote rejected request: {0}")]
    RemoteRejected(String),
    #[error("remote state unknown: {0}")]
    RemoteUnknown(String),
    #[error("authentication failed")]
    AuthenticationFailed,
    #[error("signing unavailable")]
    SigningUnavailable,
    #[error("gateway is intentionally disabled in scaffold mode")]
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanOrder {
    pub execution_id: String,
    pub account_id: AccountId,
    pub token_id: TokenId,
    pub limit_price: String,
    pub size: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostOrderAck {
    pub remote_order_id: RemoteOrderId,
    pub accepted_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteOrder {
    pub remote_order_id: RemoteOrderId,
    pub account_id: AccountId,
    pub state: String,
}

#[async_trait]
pub trait Signer: Send + Sync {
    async fn sign_order(&self, order: &PlanOrder) -> Result<SignedOrderEnvelope, GatewayError>;
}

#[async_trait]
pub trait ClobGateway: Send + Sync {
    async fn post_order(&self, order: &SignedOrderEnvelope) -> Result<PostOrderAck, GatewayError>;
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignerBackendKind {
    Disabled,
    DeterministicTest,
    OfficialSdkLocal,
    OfficialSdkRemoteKms,
    OfficialSdkExternal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignerProviderConfig {
    pub backend: SignerBackendKind,
    pub allow_local_private_key_material: bool,
    pub require_remote_signer_in_production: bool,
}

impl Default for SignerProviderConfig {
    fn default() -> Self {
        Self {
            backend: SignerBackendKind::Disabled,
            allow_local_private_key_material: false,
            require_remote_signer_in_production: true,
        }
    }
}

#[async_trait]
pub trait SignerProvider: Send + Sync {
    async fn signer_for_account(
        &self,
        account_id: &AccountId,
    ) -> Result<Arc<dyn Signer>, GatewayError>;
}

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

#[derive(Default)]
pub struct DeterministicTestSignerProvider;

#[async_trait]
impl SignerProvider for DeterministicTestSignerProvider {
    async fn signer_for_account(
        &self,
        _account_id: &AccountId,
    ) -> Result<Arc<dyn Signer>, GatewayError> {
        Ok(Arc::new(DeterministicTestSigner))
    }
}

pub struct DisabledSigner;

#[async_trait]
impl Signer for DisabledSigner {
    async fn sign_order(&self, _order: &PlanOrder) -> Result<SignedOrderEnvelope, GatewayError> {
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

pub struct DeterministicTestSigner;

#[async_trait]
impl Signer for DeterministicTestSigner {
    async fn sign_order(&self, order: &PlanOrder) -> Result<SignedOrderEnvelope, GatewayError> {
        Ok(SignedOrderEnvelope {
            internal_order_id: InternalOrderId(format!("test-order-{}", order.execution_id)),
            account_id: order.account_id.clone(),
            signer_fingerprint: "deterministic-test-signer".into(),
            signed_payload_ref: "test-only-no-real-signature".into(),
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FakeGatewayFailure {
    #[default]
    None,
    RemoteRejected(String),
    RemoteUnknown(String),
    AuthenticationFailed,
}

impl FakeGatewayFailure {
    fn apply(&self) -> Result<(), GatewayError> {
        match self {
            Self::None => Ok(()),
            Self::RemoteRejected(reason) => Err(GatewayError::RemoteRejected(reason.clone())),
            Self::RemoteUnknown(reason) => Err(GatewayError::RemoteUnknown(reason.clone())),
            Self::AuthenticationFailed => Err(GatewayError::AuthenticationFailed),
        }
    }
}

#[derive(Default)]
struct FakeGatewayInner {
    orders: HashMap<String, RemoteOrder>,
    post_failure: FakeGatewayFailure,
    cancel_failure: FakeGatewayFailure,
    read_failure: FakeGatewayFailure,
}

#[derive(Default, Clone)]
pub struct FakeGateway {
    inner: Arc<Mutex<FakeGatewayInner>>,
}

impl FakeGateway {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_post_failure(self, failure: FakeGatewayFailure) -> Self {
        self.inner
            .lock()
            .expect("fake gateway mutex poisoned")
            .post_failure = failure;
        self
    }

    pub fn with_cancel_failure(self, failure: FakeGatewayFailure) -> Self {
        self.inner
            .lock()
            .expect("fake gateway mutex poisoned")
            .cancel_failure = failure;
        self
    }

    pub fn with_read_failure(self, failure: FakeGatewayFailure) -> Self {
        self.inner
            .lock()
            .expect("fake gateway mutex poisoned")
            .read_failure = failure;
        self
    }
}

#[async_trait]
impl ClobGateway for FakeGateway {
    async fn post_order(&self, order: &SignedOrderEnvelope) -> Result<PostOrderAck, GatewayError> {
        let mut lock = self.inner.lock().expect("fake gateway mutex poisoned");
        lock.post_failure.apply()?;
        let remote_order_id = RemoteOrderId(format!("remote-{}", order.internal_order_id.0));
        let remote = RemoteOrder {
            remote_order_id: remote_order_id.clone(),
            account_id: order.account_id.clone(),
            state: "OPEN".into(),
        };
        lock.orders.insert(remote_order_id.0.clone(), remote);
        Ok(PostOrderAck {
            remote_order_id,
            accepted_at_ms: 0,
        })
    }

    async fn cancel_order(
        &self,
        account_id: &AccountId,
        remote_order_id: &RemoteOrderId,
    ) -> Result<CancelState, GatewayError> {
        let mut lock = self.inner.lock().expect("fake gateway mutex poisoned");
        lock.cancel_failure.apply()?;
        match lock.orders.get_mut(&remote_order_id.0) {
            Some(order) if &order.account_id == account_id => {
                order.state = "CANCEL_REQUESTED".into();
                Ok(CancelState::RemoteAccepted)
            }
            _ => Ok(CancelState::ReconcileRequired),
        }
    }

    async fn get_order(
        &self,
        account_id: &AccountId,
        remote_order_id: &RemoteOrderId,
    ) -> Result<Option<RemoteOrder>, GatewayError> {
        let lock = self.inner.lock().expect("fake gateway mutex poisoned");
        lock.read_failure.apply()?;
        Ok(lock
            .orders
            .get(&remote_order_id.0)
            .filter(|order| &order.account_id == account_id)
            .cloned())
    }

    async fn get_open_orders(
        &self,
        account_id: &AccountId,
    ) -> Result<Vec<RemoteOrder>, GatewayError> {
        let lock = self.inner.lock().expect("fake gateway mutex poisoned");
        lock.read_failure.apply()?;
        Ok(lock
            .orders
            .values()
            .filter(|&o| &o.account_id == account_id && o.state == "OPEN")
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_order() -> PlanOrder {
        PlanOrder {
            execution_id: "exec-gateway-test".into(),
            account_id: AccountId("acct-gateway-test".into()),
            token_id: TokenId("token-gateway-test".into()),
            limit_price: "0.5".into(),
            size: "10".into(),
        }
    }

    #[tokio::test]
    async fn deterministic_signer_provider_posts_reads_and_cancels() {
        let provider = DeterministicTestSignerProvider;
        let gateway = FakeGateway::new();
        let account = AccountId("acct-gateway-test".into());
        let signer = provider
            .signer_for_account(&account)
            .await
            .expect("test signer");
        let signed = signer.sign_order(&sample_order()).await.expect("signed");
        let ack = gateway.post_order(&signed).await.expect("posted");
        let read = gateway
            .get_order(&account, &ack.remote_order_id)
            .await
            .expect("read")
            .expect("remote order");
        assert_eq!(read.state, "OPEN");
        assert_eq!(
            gateway.get_open_orders(&account).await.expect("open").len(),
            1
        );
        let cancel = gateway
            .cancel_order(&account, &ack.remote_order_id)
            .await
            .expect("cancel");
        assert_eq!(cancel, CancelState::RemoteAccepted);
        assert!(
            gateway
                .get_open_orders(&account)
                .await
                .expect("open")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn fake_gateway_cancel_maps_to_order_lifecycle_state_machine() {
        let provider = DeterministicTestSignerProvider;
        let gateway = FakeGateway::new();
        let account = AccountId("acct-gateway-test".into());
        let signer = provider
            .signer_for_account(&account)
            .await
            .expect("test signer");
        let signed = signer.sign_order(&sample_order()).await.expect("signed");

        let mut state = OrderLifecycleState::Planned;
        state = transition_order_state(state, OrderEventKind::Signed).expect("signed transition");
        state = transition_order_state(state, OrderEventKind::PostRequested)
            .expect("post requested transition");

        let ack = gateway.post_order(&signed).await.expect("posted");
        state = transition_order_state(state, OrderEventKind::RemotePosted)
            .expect("remote posted transition");

        let cancel = gateway
            .cancel_order(&account, &ack.remote_order_id)
            .await
            .expect("cancel");
        assert_eq!(cancel, CancelState::RemoteAccepted);
        state = transition_order_state(state, OrderEventKind::CancelRequested)
            .expect("cancel requested transition");
        state = transition_order_state(state, OrderEventKind::CancelRemoteAccepted)
            .expect("cancel accepted transition");

        assert_eq!(state, OrderLifecycleState::CancelRemoteAccepted);
    }

    #[tokio::test]
    async fn fake_gateway_surfaces_remote_unknown_without_local_success() {
        let gateway = FakeGateway::new().with_post_failure(FakeGatewayFailure::RemoteUnknown(
            "timeout after signing".into(),
        ));
        let signed = DeterministicTestSigner
            .sign_order(&sample_order())
            .await
            .expect("signed");
        let err = gateway
            .post_order(&signed)
            .await
            .expect_err("remote unknown");
        assert_eq!(
            err,
            GatewayError::RemoteUnknown("timeout after signing".into())
        );
    }

    #[tokio::test]
    async fn disabled_signer_provider_refuses_to_materialize_signer() {
        let provider = DisabledSignerProvider;
        let result = provider
            .signer_for_account(&AccountId("acct-disabled".into()))
            .await;
        match result {
            Err(err) => assert_eq!(err, GatewayError::SigningUnavailable),
            Ok(_) => panic!("disabled provider must fail"),
        }
    }

    #[tokio::test]
    async fn fake_gateway_is_account_scoped() {
        let gateway = FakeGateway::new();
        let account_a = AccountId("acct-a".into());
        let account_b = AccountId("acct-b".into());
        let signer = DeterministicTestSigner;
        let mut order = sample_order();
        order.account_id = account_a.clone();
        let signed = signer.sign_order(&order).await.expect("signed");
        let ack = gateway.post_order(&signed).await.expect("posted");

        assert!(
            gateway
                .get_order(&account_b, &ack.remote_order_id)
                .await
                .expect("read")
                .is_none()
        );
        assert!(
            gateway
                .get_open_orders(&account_b)
                .await
                .expect("open")
                .is_empty()
        );
        assert_eq!(
            gateway
                .cancel_order(&account_b, &ack.remote_order_id)
                .await
                .expect("cancel foreign"),
            CancelState::ReconcileRequired
        );
        assert_eq!(
            gateway
                .get_open_orders(&account_a)
                .await
                .expect("open account a")
                .len(),
            1
        );
    }

    #[test]
    fn signer_provider_defaults_are_production_conservative() {
        let cfg = SignerProviderConfig::default();
        assert_eq!(cfg.backend, SignerBackendKind::Disabled);
        assert!(!cfg.allow_local_private_key_material);
        assert!(cfg.require_remote_signer_in_production);
    }
}
