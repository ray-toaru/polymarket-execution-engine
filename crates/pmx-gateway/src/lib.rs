mod disabled;
mod error;
mod fake;
mod model;
mod operations;
mod signer;
#[cfg(test)]
mod tests;
mod traits;

pub use disabled::{DisabledGateway, DisabledSigner, DisabledSignerProvider};
pub use error::GatewayError;
pub use fake::{FakeGateway, FakeGatewayFailure};
pub use model::{
    LiveReadErrorCategory, LiveReadNormalizedEvent, LiveReadOperation, LiveReadOutcome, PlanOrder,
    PostOrderAck, RemoteOrder, RemoteReconcileObservation, RemoteReconcileReadReport,
    RemoteReconcileReadRequest,
};
pub use operations::{
    AlertEvent, AlertSink, DeploymentReadiness, DeploymentReadinessProvider,
    DisabledOperationalPorts, ProductionGatewayAssemblyDecision, ProductionGatewayAssemblyRequest,
    SecretProvider, SecretReference,
};
pub use signer::{
    DeterministicTestSigner, DeterministicTestSignerProvider, SignerBackendKind,
    SignerProviderConfig,
};
pub use traits::{ClobGateway, MarketDataReader, RemoteReconcileReader, Signer, SignerProvider};
