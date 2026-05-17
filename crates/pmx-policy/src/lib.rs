use pmx_core::{
    BlockReason, CollateralProfileStatus, ConstraintDecision, DecisionStatus, FeasibilitySnapshot,
    GeoblockStatus, HashValue, NormalizedIntent, QuantityBound, RuntimeStateSummary, WorkerStatus,
};

pub fn evaluate_constraints(
    intent: &NormalizedIntent,
    snapshot: &FeasibilitySnapshot,
) -> ConstraintDecision {
    let mut reasons = Vec::new();
    collect_runtime_reasons(&snapshot.runtime_state, &mut reasons);

    if matches!(intent.quantity_bound, QuantityBound::Unsupported(_)) {
        reasons.push(BlockReason::UnsupportedQuantityBound);
    }

    let status = if reasons.is_empty() {
        DecisionStatus::Allow
    } else {
        DecisionStatus::Block
    };

    ConstraintDecision {
        decision_id: format!("decision-{}", snapshot.snapshot_id),
        decision_hash: HashValue(format!("decision-hash-{}", snapshot.snapshot_hash.0)),
        status,
        reasons,
    }
}

fn collect_runtime_reasons(state: &RuntimeStateSummary, reasons: &mut Vec<BlockReason>) {
    if state.kill_switch_enabled {
        reasons.push(BlockReason::KillSwitchOn);
    }

    match state.geoblock_status {
        GeoblockStatus::Allowed => {}
        GeoblockStatus::Blocked => reasons.push(BlockReason::GeoblockBlocked),
        GeoblockStatus::Unknown => reasons.push(BlockReason::GeoblockUnknown),
        GeoblockStatus::Error => reasons.push(BlockReason::GeoblockError),
    }

    match state.worker_status {
        WorkerStatus::Healthy => {}
        WorkerStatus::Degraded => reasons.push(BlockReason::WorkerDegraded),
        WorkerStatus::Stale => reasons.push(BlockReason::WorkerStale),
        WorkerStatus::Unknown => reasons.push(BlockReason::WorkerUnknown),
    }

    match state.collateral_profile_status {
        CollateralProfileStatus::Resolved | CollateralProfileStatus::DefaultResolved => {}
        CollateralProfileStatus::ExplicitMissing => {
            reasons.push(BlockReason::CollateralProfileMissing)
        }
        CollateralProfileStatus::Unknown => reasons.push(BlockReason::CollateralProfileUnknown),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use pmx_core::*;

    fn intent() -> NormalizedIntent {
        NormalizedIntent {
            normalized_intent_id: "n1".into(),
            intent_hash: HashValue("h1".into()),
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
