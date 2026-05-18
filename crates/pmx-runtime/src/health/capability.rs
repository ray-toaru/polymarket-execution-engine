use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthLevel {
    Healthy,
    Degraded,
    Stale,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityHealth {
    pub capability: String,
    pub required_for_submit: bool,
    pub level: HealthLevel,
    pub last_observed_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

impl CapabilityHealth {
    pub fn blocks_submit(&self) -> bool {
        self.required_for_submit
            && matches!(
                self.level,
                HealthLevel::Degraded | HealthLevel::Stale | HealthLevel::Unknown
            )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeHealthBreakdown {
    pub account_id: String,
    pub account_capabilities: Vec<CapabilityHealth>,
    pub market_capabilities: Vec<CapabilityHealth>,
    pub asset_capabilities: Vec<CapabilityHealth>,
    pub worker_capabilities: Vec<CapabilityHealth>,
}

impl RuntimeHealthBreakdown {
    pub fn blocking_capabilities(&self) -> Vec<&CapabilityHealth> {
        self.account_capabilities
            .iter()
            .chain(self.market_capabilities.iter())
            .chain(self.asset_capabilities.iter())
            .chain(self.worker_capabilities.iter())
            .filter(|health| health.blocks_submit())
            .collect()
    }

    pub fn all_capabilities(&self) -> Vec<&CapabilityHealth> {
        self.account_capabilities
            .iter()
            .chain(self.market_capabilities.iter())
            .chain(self.asset_capabilities.iter())
            .chain(self.worker_capabilities.iter())
            .collect()
    }
}
