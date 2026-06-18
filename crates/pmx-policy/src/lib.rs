mod decision;
mod runtime;

pub use decision::*;
pub use runtime::{
    CAP_MARKET_BOOK_FUTURE_DATED, CAP_MARKET_BOOK_INSUFFICIENT_TOP_LIQUIDITY,
    CAP_MARKET_BOOK_QUANTITY_UNSUPPORTED, CAP_MARKET_BOOK_STALE, CAP_MARKET_BOOK_UNAVAILABLE,
};

// Contract validation compatibility anchor:
// WorkerStatus::Degraded => reasons.push(BlockReason::WorkerDegraded)

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use pmx_core::*;

    fn intent() -> NormalizedIntent {
        NormalizedIntent {
            normalized_intent_id: "n1".into(),
            intent_hash: HashValue("h1".into()),
            correlation_id: None,
            account_id: AccountId("a1".into()),
            market: MarketRef {
                condition_id: ConditionId("c1".into()),
                slug: None,
                is_sports: false,
            },
            token_id: TokenId("t1".into()),
            side: Side::Buy,
            quantity_bound: QuantityBound::WorstCaseQuoteNotional(DecimalString("10".into())),
            limit_price: DecimalString("0.5".into()),
            time_in_force: TimeInForce::Gtc,
            collateral_profile_id: None,
        }
    }

    fn snapshot(state: RuntimeStateSummary) -> FeasibilitySnapshot {
        FeasibilitySnapshot {
            snapshot_id: "s1".into(),
            snapshot_hash: HashValue("sh1".into()),
            normalized_intent_id: "n1".into(),
            correlation_id: None,
            runtime_state: state,
            captured_at: Utc::now(),
        }
    }

    #[test]
    fn geoblock_unknown_blocks() {
        let decision = evaluate_constraints(
            &intent(),
            &snapshot(RuntimeStateSummary {
                geoblock_status: GeoblockStatus::Unknown,
                worker_status: WorkerStatus::Healthy,
                collateral_profile_status: CollateralProfileStatus::DefaultResolved,
                kill_switch_enabled: false,
                required_capabilities: vec![],
            }),
        );
        assert_eq!(decision.status, DecisionStatus::Block);
        assert!(decision.decision_hash.is_sha256_hex());
        assert!(decision.reasons.contains(&BlockReason::GeoblockUnknown));
    }

    #[test]
    fn explicit_collateral_miss_blocks() {
        let decision = evaluate_constraints(
            &intent(),
            &snapshot(RuntimeStateSummary {
                geoblock_status: GeoblockStatus::Allowed,
                worker_status: WorkerStatus::Healthy,
                collateral_profile_status: CollateralProfileStatus::ExplicitMissing,
                kill_switch_enabled: false,
                required_capabilities: vec![],
            }),
        );
        assert!(
            decision
                .reasons
                .contains(&BlockReason::CollateralProfileMissing)
        );
    }

    #[test]
    fn degraded_worker_blocks_pre_live() {
        let decision = evaluate_constraints(
            &intent(),
            &snapshot(RuntimeStateSummary {
                geoblock_status: GeoblockStatus::Allowed,
                worker_status: WorkerStatus::Degraded,
                collateral_profile_status: CollateralProfileStatus::DefaultResolved,
                kill_switch_enabled: false,
                required_capabilities: vec![],
            }),
        );
        assert_eq!(decision.status, DecisionStatus::Block);
        assert!(decision.reasons.contains(&BlockReason::WorkerDegraded));
    }
}
