use super::*;
use pmx_core::{AccountId, DecimalString, ExposureProjection, PortfolioProjection};

fn projection(account_id: String, observed_at_ms: i64, gross: &str) -> PortfolioProjection {
    PortfolioProjection {
        account_id: AccountId(account_id),
        fills: vec![],
        positions: vec![],
        open_orders: vec![],
        exposure: ExposureProjection {
            gross_notional: DecimalString(gross.into()),
            open_order_notional: DecimalString("0".into()),
        },
        observed_at_ms,
    }
}

#[tokio::test]
async fn portfolio_projection_round_trips_and_rejects_stale_overwrite() {
    let Some(store) = test_store().await else {
        return;
    };
    let account_id = unique("acct-portfolio");
    let current = projection(account_id.clone(), 200, "2");
    store
        .save_portfolio_projection(&current)
        .await
        .expect("save current projection");

    store
        .save_portfolio_projection(&projection(account_id.clone(), 100, "1"))
        .await
        .expect("stale save is an idempotent no-op");
    store
        .save_portfolio_projection(&projection(account_id.clone(), 200, "9"))
        .await
        .expect("same timestamp save is an idempotent no-op");

    assert_eq!(
        store
            .load_portfolio_projection(&AccountId(account_id))
            .await
            .expect("load projection"),
        current
    );
}

#[tokio::test]
async fn portfolio_projection_is_isolated_by_account() {
    let Some(store) = test_store().await else {
        return;
    };
    let left = projection(unique("acct-left"), 100, "1");
    let right = projection(unique("acct-right"), 100, "2");
    store.save_portfolio_projection(&left).await.unwrap();
    store.save_portfolio_projection(&right).await.unwrap();

    assert_eq!(
        store
            .load_portfolio_projection(&left.account_id)
            .await
            .unwrap(),
        left
    );
    assert_eq!(
        store
            .load_portfolio_projection(&right.account_id)
            .await
            .unwrap(),
        right
    );
}
