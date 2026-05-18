use pmx_core::{
    BlockReason, CollateralProfileStatus, GeoblockStatus, RuntimeStateSummary, WorkerStatus,
};

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
}
