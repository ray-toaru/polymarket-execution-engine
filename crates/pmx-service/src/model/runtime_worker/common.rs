use pmx_runtime::RuntimeSignal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkerTick {
    pub worker_id: String,
    pub role: String,
    pub capability: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(default)]
    pub signals: Vec<RuntimeSignal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkerTickReceipt {
    pub worker_id: String,
    pub capability: String,
    pub heartbeat_recorded: bool,
    pub observations_recorded: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkerProviderTickReceipt {
    pub worker_id: String,
    pub provider_name: String,
    pub lease_owner_active: bool,
    pub submit_allowed_by_runtime: bool,
    pub heartbeat_recorded: bool,
    pub observations_recorded: usize,
}
