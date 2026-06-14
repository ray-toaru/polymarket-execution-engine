use super::*;

fn trade_intent(side: Side) -> TradeIntent {
    base_intent(
        side,
        QuantityIntent {
            max_notional: None,
            max_shares: Some(DecimalString("2".into())),
        },
    )
}

#[test]
fn execution_commands_model_place_cancel_and_replace_without_remote_effects() {
    let place = ExecutionCommand::Place {
        intent: trade_intent(Side::Buy),
    };
    let cancel = ExecutionCommand::Cancel {
        account_id: AccountId("acct-1".into()),
        order_id: InternalOrderId("order-1".into()),
    };
    let replace = ExecutionCommand::Replace {
        account_id: AccountId("acct-1".into()),
        order_id: InternalOrderId("order-1".into()),
        replacement: trade_intent(Side::Sell),
    };

    assert_eq!(place.kind(), ExecutionCommandKind::Place);
    assert_eq!(cancel.kind(), ExecutionCommandKind::Cancel);
    assert_eq!(replace.kind(), ExecutionCommandKind::Replace);
    assert!(!place.authorizes_remote_side_effect());
    assert!(!cancel.authorizes_remote_side_effect());
    assert!(!replace.authorizes_remote_side_effect());
}

#[test]
fn portfolio_projection_keeps_fills_positions_open_orders_and_exposure_typed() {
    let projection = PortfolioProjection {
        account_id: AccountId("acct-1".into()),
        fills: vec![FillRecord {
            fill_id: "fill-1".into(),
            order_id: InternalOrderId("order-1".into()),
            token_id: TokenId("token-1".into()),
            side: Side::Buy,
            price: DecimalString("0.51".into()),
            shares: DecimalString("2".into()),
            observed_at_ms: 1_000,
        }],
        positions: vec![PositionProjection {
            token_id: TokenId("token-1".into()),
            shares: DecimalString("2".into()),
            average_price: DecimalString("0.51".into()),
        }],
        open_orders: vec![OpenOrderProjection {
            order_id: InternalOrderId("order-1".into()),
            token_id: TokenId("token-1".into()),
            side: Side::Buy,
            remaining_shares: DecimalString("1".into()),
            limit_price: DecimalString("0.51".into()),
        }],
        exposure: ExposureProjection {
            gross_notional: DecimalString("1.02".into()),
            open_order_notional: DecimalString("0.51".into()),
        },
        observed_at_ms: 1_000,
    };

    assert_eq!(projection.fills.len(), 1);
    assert_eq!(projection.positions[0].shares, DecimalString("2".into()));
    assert_eq!(projection.open_orders[0].side, Side::Buy);
    assert_eq!(
        projection.exposure.gross_notional,
        DecimalString("1.02".into())
    );
}

#[test]
fn market_book_snapshot_fails_closed_when_stale_or_future_dated() {
    let fresh = MarketBookSnapshot {
        condition_id: ConditionId("cond-1".into()),
        token_id: TokenId("token-1".into()),
        bids: vec![BookLevel {
            price: DecimalString("0.50".into()),
            shares: DecimalString("10".into()),
        }],
        asks: vec![BookLevel {
            price: DecimalString("0.51".into()),
            shares: DecimalString("8".into()),
        }],
        observed_at_ms: 1_000,
        valid_for_ms: 500,
    };

    assert_eq!(fresh.freshness_at(1_250), MarketDataFreshness::Fresh);
    assert_eq!(fresh.freshness_at(1_501), MarketDataFreshness::Stale);
    assert_eq!(fresh.freshness_at(999), MarketDataFreshness::FutureDated);
    assert!(fresh.require_fresh_at(1_250).is_ok());
    assert!(matches!(
        fresh.require_fresh_at(1_501),
        Err(CoreError::StaleMarketData)
    ));
    assert!(matches!(
        fresh.require_fresh_at(999),
        Err(CoreError::FutureDatedMarketData)
    ));
}

#[test]
fn risk_limits_block_kill_switch_and_excess_exposure() {
    let limits = RiskLimits {
        max_gross_notional: DecimalString("10".into()),
        max_open_order_notional: DecimalString("4".into()),
        kill_switch_active: false,
    };
    let within_limits = ExposureProjection {
        gross_notional: DecimalString("9.5".into()),
        open_order_notional: DecimalString("3".into()),
    };
    let over_limits = ExposureProjection {
        gross_notional: DecimalString("10.01".into()),
        open_order_notional: DecimalString("3".into()),
    };

    assert_eq!(
        assess_exposure(&within_limits, &limits).unwrap(),
        RiskDecision::Allow
    );
    assert_eq!(
        assess_exposure(&over_limits, &limits).unwrap(),
        RiskDecision::Block(RiskBlockReason::GrossExposureExceeded)
    );
    assert_eq!(
        assess_exposure(
            &within_limits,
            &RiskLimits {
                kill_switch_active: true,
                ..limits
            }
        )
        .unwrap(),
        RiskDecision::Block(RiskBlockReason::KillSwitchActive)
    );
}
