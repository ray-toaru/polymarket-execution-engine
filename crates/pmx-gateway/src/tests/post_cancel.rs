use super::*;

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
