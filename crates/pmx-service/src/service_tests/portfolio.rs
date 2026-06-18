use super::*;

fn portfolio_projection() -> PortfolioProjection {
    PortfolioProjection {
        account_id: AccountId("acct-portfolio-service".into()),
        fills: vec![FillRecord {
            fill_id: "fill-1".into(),
            order_id: InternalOrderId("order-1".into()),
            token_id: TokenId("token-1".into()),
            side: Side::Buy,
            price: DecimalString("0.50".into()),
            shares: DecimalString("2".into()),
            observed_at_ms: 1_000,
        }],
        positions: vec![PositionProjection {
            token_id: TokenId("token-1".into()),
            shares: DecimalString("2".into()),
            average_price: DecimalString("0.50".into()),
        }],
        open_orders: vec![OpenOrderProjection {
            order_id: InternalOrderId("order-2".into()),
            token_id: TokenId("token-2".into()),
            side: Side::Sell,
            remaining_shares: DecimalString("3".into()),
            limit_price: DecimalString("0.60".into()),
        }],
        exposure: ExposureProjection {
            gross_notional: DecimalString("1.00".into()),
            open_order_notional: DecimalString("1.80".into()),
        },
        observed_at_ms: 2_000,
    }
}

#[tokio::test]
async fn service_records_portfolio_projection_and_assesses_risk_limits() {
    let service = ExecutorService::new(InMemoryStore::default());
    let projection = portfolio_projection();

    service
        .record_portfolio_projection(projection.clone())
        .await
        .expect("record portfolio projection");

    assert_eq!(
        service
            .load_portfolio_projection(&projection.account_id)
            .await
            .expect("load projection"),
        projection
    );

    let allow = service
        .assess_portfolio_risk(
            &projection.account_id,
            RiskLimits {
                max_gross_notional: DecimalString("2".into()),
                max_open_order_notional: DecimalString("2".into()),
                kill_switch_active: false,
            },
        )
        .await
        .expect("risk allow");
    assert_eq!(allow, RiskDecision::Allow);

    let block = service
        .assess_portfolio_risk(
            &projection.account_id,
            RiskLimits {
                max_gross_notional: DecimalString("2".into()),
                max_open_order_notional: DecimalString("1".into()),
                kill_switch_active: false,
            },
        )
        .await
        .expect("risk block");
    assert_eq!(
        block,
        RiskDecision::Block(RiskBlockReason::OpenOrderExposureExceeded)
    );
}
