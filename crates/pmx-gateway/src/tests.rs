use super::*;
use pmx_core::{OrderEventKind, OrderLifecycleState, transition_order_state};

fn sample_order() -> PlanOrder {
    PlanOrder {
        execution_id: "exec-gateway-test".into(),
        account_id: pmx_core::AccountId("acct-gateway-test".into()),
        token_id: pmx_core::TokenId("token-gateway-test".into()),
        limit_price: "0.5".into(),
        size: "10".into(),
    }
}

#[tokio::test]
async fn deterministic_signer_provider_posts_reads_and_cancels() {
    let provider = DeterministicTestSignerProvider;
    let gateway = FakeGateway::new();
    let account = pmx_core::AccountId("acct-gateway-test".into());
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
    assert_eq!(cancel, pmx_core::CancelState::RemoteAccepted);
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
    let account = pmx_core::AccountId("acct-gateway-test".into());
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
    assert_eq!(cancel, pmx_core::CancelState::RemoteAccepted);
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
        .signer_for_account(&pmx_core::AccountId("acct-disabled".into()))
        .await;
    match result {
        Err(err) => assert_eq!(err, GatewayError::SigningUnavailable),
        Ok(_) => panic!("disabled provider must fail"),
    }
}

#[tokio::test]
async fn fake_gateway_is_account_scoped() {
    let gateway = FakeGateway::new();
    let account_a = pmx_core::AccountId("acct-a".into());
    let account_b = pmx_core::AccountId("acct-b".into());
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
        pmx_core::CancelState::ReconcileRequired
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

#[tokio::test]
async fn fake_remote_reconcile_reader_is_read_only_and_account_scoped() {
    let gateway = FakeGateway::new().with_post_failure(FakeGatewayFailure::RemoteRejected(
        "post path must not be used by reconcile reader".into(),
    ));
    let account = pmx_core::AccountId("acct-reconcile-reader".into());
    let foreign_account = pmx_core::AccountId("acct-foreign".into());
    let open_id = pmx_core::RemoteOrderId("remote-open".into());
    let foreign_id = pmx_core::RemoteOrderId("remote-foreign".into());
    let missing_id = pmx_core::RemoteOrderId("remote-missing".into());
    gateway.insert_remote_order_for_test(RemoteOrder {
        remote_order_id: open_id.clone(),
        account_id: account.clone(),
        state: "OPEN".into(),
    });
    gateway.insert_remote_order_for_test(RemoteOrder {
        remote_order_id: foreign_id.clone(),
        account_id: foreign_account,
        state: "OPEN".into(),
    });

    let report = gateway
        .read_remote_order_observations(&RemoteReconcileReadRequest {
            account_id: account,
            remote_order_ids: vec![open_id.clone(), foreign_id.clone(), missing_id.clone()],
            no_trading_side_effect: true,
        })
        .await
        .expect("read-only reconcile report");

    assert!(report.no_trading_side_effect);
    assert_eq!(
        report.observations,
        vec![
            RemoteReconcileObservation {
                remote_order_id: open_id,
                observation: pmx_core::RemoteOrderObservation::Open,
                remote_state: Some("OPEN".into()),
            },
            RemoteReconcileObservation {
                remote_order_id: foreign_id,
                observation: pmx_core::RemoteOrderObservation::Missing,
                remote_state: None,
            },
            RemoteReconcileObservation {
                remote_order_id: missing_id,
                observation: pmx_core::RemoteOrderObservation::Missing,
                remote_state: None,
            },
        ]
    );
}

#[tokio::test]
async fn remote_reconcile_reader_rejects_side_effect_requests() {
    let gateway = FakeGateway::new();
    let err = gateway
        .read_remote_order_observations(&RemoteReconcileReadRequest {
            account_id: pmx_core::AccountId("acct-reconcile-reader".into()),
            remote_order_ids: vec![pmx_core::RemoteOrderId("remote-open".into())],
            no_trading_side_effect: false,
        })
        .await
        .expect_err("side-effect-capable request must be rejected");

    assert_eq!(
        err,
        GatewayError::RemoteRejected(
            "remote reconcile read must be marked no-trading-side-effect".into()
        )
    );
}

#[tokio::test]
async fn disabled_gateway_reconcile_reader_is_disabled() {
    let err = DisabledGateway
        .read_remote_order_observations(&RemoteReconcileReadRequest {
            account_id: pmx_core::AccountId("acct-reconcile-reader".into()),
            remote_order_ids: vec![pmx_core::RemoteOrderId("remote-open".into())],
            no_trading_side_effect: true,
        })
        .await
        .expect_err("disabled gateway must not read remote state");

    assert_eq!(err, GatewayError::Disabled);
}
