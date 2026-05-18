use crate::{
    ClobGateway, GatewayError, PostOrderAck, RemoteOrder, RemoteReconcileObservation,
    RemoteReconcileReadReport, RemoteReconcileReadRequest, RemoteReconcileReader,
};
use async_trait::async_trait;
use pmx_core::{
    AccountId, CancelState, RemoteOrderId, RemoteOrderObservation, SignedOrderEnvelope,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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

    pub fn insert_remote_order_for_test(&self, order: RemoteOrder) {
        self.inner
            .lock()
            .expect("fake gateway mutex poisoned")
            .orders
            .insert(order.remote_order_id.0.clone(), order);
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

fn remote_state_to_observation(remote_state: &str) -> RemoteOrderObservation {
    match remote_state {
        "MISSING" => RemoteOrderObservation::Missing,
        "UNKNOWN" => RemoteOrderObservation::Unknown,
        _ => RemoteOrderObservation::Open,
    }
}

#[async_trait]
impl RemoteReconcileReader for FakeGateway {
    async fn read_remote_order_observations(
        &self,
        request: &RemoteReconcileReadRequest,
    ) -> Result<RemoteReconcileReadReport, GatewayError> {
        if !request.no_trading_side_effect {
            return Err(GatewayError::RemoteRejected(
                "remote reconcile read must be marked no-trading-side-effect".into(),
            ));
        }

        let mut observations = Vec::with_capacity(request.remote_order_ids.len());
        for remote_order_id in &request.remote_order_ids {
            let remote = self
                .get_order(&request.account_id, remote_order_id)
                .await?
                .map(|order| {
                    let observation = remote_state_to_observation(&order.state);
                    RemoteReconcileObservation {
                        remote_order_id: order.remote_order_id,
                        observation,
                        remote_state: Some(order.state),
                    }
                })
                .unwrap_or_else(|| RemoteReconcileObservation {
                    remote_order_id: remote_order_id.clone(),
                    observation: RemoteOrderObservation::Missing,
                    remote_state: None,
                });
            observations.push(remote);
        }

        Ok(RemoteReconcileReadReport {
            observations,
            no_trading_side_effect: true,
        })
    }
}
