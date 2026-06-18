use pmx_core::{
    BlockReason, CollateralProfileStatus, GeoblockStatus, RuntimeStateSummary, WorkerStatus,
};

pub const CAP_MARKET_BOOK_STALE: &str = "market_book_stale";
pub const CAP_MARKET_BOOK_FUTURE_DATED: &str = "market_book_future_dated";
pub const CAP_MARKET_BOOK_UNAVAILABLE: &str = "market_book_unavailable";
pub const CAP_MARKET_BOOK_INSUFFICIENT_TOP_LIQUIDITY: &str =
    "market_book_insufficient_top_liquidity";
pub const CAP_MARKET_BOOK_QUANTITY_UNSUPPORTED: &str = "market_book_quantity_unsupported";

pub(crate) fn collect_runtime_reasons(state: &RuntimeStateSummary, reasons: &mut Vec<BlockReason>) {
    // Contract validation compatibility anchor:
    // WorkerStatus::Degraded => reasons.push(BlockReason::WorkerDegraded)
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

    collect_market_data_reasons(&state.required_capabilities, reasons);
}

fn collect_market_data_reasons(required_capabilities: &[String], reasons: &mut Vec<BlockReason>) {
    for capability in required_capabilities {
        match capability.as_str() {
            CAP_MARKET_BOOK_STALE => reasons.push(BlockReason::StaleMarketData),
            CAP_MARKET_BOOK_FUTURE_DATED => reasons.push(BlockReason::FutureDatedMarketData),
            CAP_MARKET_BOOK_UNAVAILABLE => reasons.push(BlockReason::MarketBookUnavailable),
            CAP_MARKET_BOOK_INSUFFICIENT_TOP_LIQUIDITY => {
                reasons.push(BlockReason::InsufficientTopBookLiquidity)
            }
            CAP_MARKET_BOOK_QUANTITY_UNSUPPORTED => {
                reasons.push(BlockReason::MarketBookQuantityUnsupported)
            }
            _ => {}
        }
    }
}
