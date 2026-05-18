use pmx_runtime::RuntimeWorkerProviderSnapshot;
use serde::{Deserialize, Serialize};

use crate::RuntimeWorkerProviderTickReceipt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkerContinuousTick {
    pub snapshots: Vec<RuntimeWorkerProviderSnapshot>,
    pub no_trading_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkerContinuousTickReceipt {
    pub ticks_recorded: Vec<RuntimeWorkerProviderTickReceipt>,
    pub all_submit_allowed_by_runtime: bool,
    pub no_trading_side_effect: bool,
}
