use super::*;

#[tokio::test]
async fn in_memory_store_round_trips_portfolio_projection_by_account() {
    let store = InMemoryStore::default();
    let projection = pmx_core::PortfolioProjection {
        account_id: pmx_core::AccountId("acct-portfolio".into()),
        fills: vec![],
        positions: vec![],
        open_orders: vec![],
        exposure: pmx_core::ExposureProjection {
            gross_notional: pmx_core::DecimalString("0".into()),
            open_order_notional: pmx_core::DecimalString("0".into()),
        },
        observed_at_ms: 1_000,
    };

    store
        .save_portfolio_projection(&projection)
        .await
        .expect("save projection");

    assert_eq!(
        store
            .load_portfolio_projection(&projection.account_id)
            .await
            .expect("load projection"),
        projection
    );
}
