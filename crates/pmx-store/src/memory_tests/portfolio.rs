use super::*;

fn portfolio_projection(
    account_id: &str,
    observed_at_ms: i64,
    gross_notional: &str,
) -> pmx_core::PortfolioProjection {
    pmx_core::PortfolioProjection {
        account_id: pmx_core::AccountId(account_id.into()),
        fills: vec![],
        positions: vec![],
        open_orders: vec![],
        exposure: pmx_core::ExposureProjection {
            gross_notional: pmx_core::DecimalString(gross_notional.into()),
            open_order_notional: pmx_core::DecimalString("0".into()),
        },
        observed_at_ms,
    }
}

#[tokio::test]
async fn in_memory_store_round_trips_portfolio_projection_by_account() {
    let store = InMemoryStore::default();
    let projection = portfolio_projection("acct-portfolio", 1_000, "0");

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

#[tokio::test]
async fn in_memory_store_rejects_stale_portfolio_projection_overwrite() {
    let store = InMemoryStore::default();
    let current = portfolio_projection("acct-portfolio-stale", 200, "2");
    let stale = portfolio_projection("acct-portfolio-stale", 100, "1");
    let same_timestamp = portfolio_projection("acct-portfolio-stale", 200, "9");

    store
        .save_portfolio_projection(&current)
        .await
        .expect("save current projection");
    store
        .save_portfolio_projection(&stale)
        .await
        .expect("stale save is an idempotent no-op");
    store
        .save_portfolio_projection(&same_timestamp)
        .await
        .expect("same timestamp save is an idempotent no-op");

    assert_eq!(
        store
            .load_portfolio_projection(&current.account_id)
            .await
            .expect("load projection"),
        current
    );
}
