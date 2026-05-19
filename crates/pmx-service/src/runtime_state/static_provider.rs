use super::*;

#[derive(Debug, Clone)]
pub struct StaticRuntimeStateProvider {
    runtime_state: RuntimeStateSummary,
}

impl StaticRuntimeStateProvider {
    pub fn new(runtime_state: RuntimeStateSummary) -> Self {
        Self { runtime_state }
    }
}

#[async_trait]
impl RuntimeStateProvider for StaticRuntimeStateProvider {
    async fn capture_runtime_state(
        &self,
        _normalized_intent: &NormalizedIntent,
    ) -> RuntimeStateSummary {
        self.runtime_state.clone()
    }
}
