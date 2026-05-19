use super::*;

pub fn apply_runtime_worker_observations(
    mut base: RuntimeStateSummary,
    observations: &[RuntimeWorkerObservation],
) -> RuntimeStateSummary {
    if let Some(observed_status) = runtime_observation_worker_status(observations) {
        base.worker_status = worst_worker_status(base.worker_status, observed_status);
        for observation in observations {
            if !base.required_capabilities.contains(&observation.capability) {
                base.required_capabilities
                    .push(observation.capability.clone());
            }
        }
    }
    base
}
