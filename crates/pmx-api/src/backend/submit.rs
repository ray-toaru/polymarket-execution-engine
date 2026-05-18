use pmx_service::{ServiceError, SubmitOutcome, SubmitPlanCommand};

use super::ServiceBackend;

impl ServiceBackend {
    pub(crate) async fn submit_plan(
        &self,
        req: SubmitPlanCommand,
    ) -> Result<SubmitOutcome, ServiceError> {
        match self {
            Self::InMemory(service) => service.submit_plan(req).await,
            Self::Postgres(service) => service.submit_plan(req).await,
        }
    }
}
