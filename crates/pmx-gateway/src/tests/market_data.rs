use super::*;

#[tokio::test]
async fn fake_market_data_reader_returns_book_snapshot_by_condition_and_token() {
    let gateway = FakeGateway::new();
    let condition_id = pmx_core::ConditionId("cond-1".into());
    let token_id = pmx_core::TokenId("token-1".into());
    let snapshot = pmx_core::MarketBookSnapshot {
        condition_id: condition_id.clone(),
        token_id: token_id.clone(),
        bids: vec![pmx_core::BookLevel {
            price: pmx_core::DecimalString("0.49".into()),
            shares: pmx_core::DecimalString("3".into()),
        }],
        asks: vec![pmx_core::BookLevel {
            price: pmx_core::DecimalString("0.51".into()),
            shares: pmx_core::DecimalString("5".into()),
        }],
        observed_at_ms: 1_000,
        valid_for_ms: 500,
    };
    gateway.insert_market_book_for_test(snapshot.clone());

    let read = gateway
        .read_market_book(&condition_id, &token_id)
        .await
        .expect("fake book read");

    assert_eq!(read, snapshot);
    assert!(
        read.has_top_liquidity_for(
            &pmx_core::Side::Buy,
            &pmx_core::DecimalString("5".into()),
            1_250
        )
        .expect("fresh book")
    );
}

#[tokio::test]
async fn fake_market_data_reader_fails_closed_for_missing_book() {
    let err = FakeGateway::new()
        .read_market_book(
            &pmx_core::ConditionId("cond-missing".into()),
            &pmx_core::TokenId("token-missing".into()),
        )
        .await
        .expect_err("missing fake book must fail closed");

    assert_eq!(
        err,
        GatewayError::RemoteUnknown("market book snapshot not found".into())
    );
}

#[tokio::test]
async fn disabled_market_data_reader_fails_closed() {
    let err = DisabledGateway
        .read_market_book(
            &pmx_core::ConditionId("cond-1".into()),
            &pmx_core::TokenId("token-1".into()),
        )
        .await
        .expect_err("disabled gateway must not fabricate market data");

    assert_eq!(err, GatewayError::Disabled);
}
