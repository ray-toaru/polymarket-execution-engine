use super::*;
use pmx_core::{OrderEventKind, OrderLifecycleState, transition_order_state};

fn sample_order() -> PlanOrder {
    PlanOrder {
        execution_id: "exec-gateway-test".into(),
        account_id: pmx_core::AccountId("acct-gateway-test".into()),
        token_id: pmx_core::TokenId("token-gateway-test".into()),
        side: "Buy".into(),
        limit_price: "0.5".into(),
        size: "10".into(),
        time_in_force: "Gtc".into(),
    }
}

#[path = "tests/post_cancel.rs"]
mod post_cancel;

#[path = "tests/signer.rs"]
mod signer;

#[tokio::test]
async fn disabled_operational_ports_fail_closed_without_secret_material() {
    use crate::{
        AlertEvent, AlertSink, DeploymentReadinessProvider, DisabledOperationalPorts,
        SecretProvider,
    };

    let ports = DisabledOperationalPorts;
    assert!(ports.resolve_reference("clob").await.is_err());
    assert!(
        ports
            .publish(&AlertEvent {
                code: "TEST".into(),
                severity: "INFO".into(),
                correlation_id: "corr-1".into(),
                redacted_message: "redacted".into(),
            })
            .await
            .is_err()
    );
    assert!(!ports.readiness().await.unwrap().ready);
}

#[path = "tests/reconcile_reader.rs"]
mod reconcile_reader;

#[path = "tests/market_data.rs"]
mod market_data;
