use super::*;

pub fn fail_closed_runtime_state(required_capabilities: Vec<String>) -> RuntimeStateSummary {
    RuntimeStateSummary {
        geoblock_status: GeoblockStatus::Unknown,
        worker_status: WorkerStatus::Unknown,
        collateral_profile_status: CollateralProfileStatus::Unknown,
        kill_switch_enabled: true,
        required_capabilities,
    }
}

#[derive(Debug, Clone, Default)]
pub struct FailClosedRuntimeStateProvider;

#[async_trait]
impl RuntimeStateProvider for FailClosedRuntimeStateProvider {
    async fn capture_runtime_state(
        &self,
        _normalized_intent: &NormalizedIntent,
    ) -> RuntimeStateSummary {
        fail_closed_runtime_state(vec![])
    }
}
