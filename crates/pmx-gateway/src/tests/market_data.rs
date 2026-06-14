use super::*;

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
