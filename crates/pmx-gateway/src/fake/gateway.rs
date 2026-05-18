use async_trait::async_trait;
use pmx_core::{AccountId, CancelState, RemoteOrderId, SignedOrderEnvelope};

use crate::{ClobGateway, GatewayError, PostOrderAck, RemoteOrder};

use super::FakeGateway;

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
