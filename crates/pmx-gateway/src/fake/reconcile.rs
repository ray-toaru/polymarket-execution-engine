use crate::ClobGateway;
use async_trait::async_trait;
use pmx_core::RemoteOrderObservation;

use crate::{
    GatewayError, RemoteReconcileObservation, RemoteReconcileReadReport,
    RemoteReconcileReadRequest, RemoteReconcileReader,
};

use super::FakeGateway;

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
