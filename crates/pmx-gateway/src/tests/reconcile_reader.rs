use super::*;

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
