use chrono::{DateTime, Utc};
use pmx_core::GeoblockStatus;
use serde::{Deserialize, Serialize};

use super::{CapabilityHealth, HealthLevel, RuntimeHealthBreakdown};
use crate::{RuntimeWorkerKind, WebSocketChannel};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RuntimeSignal {
    WebSocket {
        channel: WebSocketChannel,
        connected: bool,
        stale: bool,
        last_observed_at: Option<DateTime<Utc>>,
        last_error: Option<String>,
    },
    HeartbeatLease {
        active: bool,
        last_observed_at: Option<DateTime<Utc>>,
        last_error: Option<String>,
    },
    Geoblock {
        status: GeoblockStatus,
        last_observed_at: Option<DateTime<Utc>>,
        last_error: Option<String>,
    },
    ResourceRefresh {
        fresh: bool,
        last_observed_at: Option<DateTime<Utc>>,
        last_error: Option<String>,
    },
    ReconcileBacklog {
        remote_unknown_orders: u32,
        last_observed_at: Option<DateTime<Utc>>,
        last_error: Option<String>,
    },
}

impl RuntimeSignal {
    pub fn to_capability_health(&self) -> CapabilityHealth {
        match self {
            RuntimeSignal::WebSocket {
                channel,
                connected,
                stale,
                last_observed_at,
                last_error,
            } => {
                let level = if *connected && !*stale {
                    HealthLevel::Healthy
                } else if *connected && *stale {
                    HealthLevel::Stale
                } else {
                    HealthLevel::Degraded
                };
                CapabilityHealth {
                    capability: format!("websocket:{channel:?}").to_ascii_lowercase(),
                    required_for_submit: true,
                    level,
                    last_observed_at: *last_observed_at,
                    last_error: last_error.clone(),
                }
            }
            RuntimeSignal::HeartbeatLease {
                active,
                last_observed_at,
                last_error,
            } => CapabilityHealth {
                capability: "heartbeat-lease".into(),
                required_for_submit: true,
                level: if *active {
                    HealthLevel::Healthy
                } else {
                    HealthLevel::Stale
                },
                last_observed_at: *last_observed_at,
                last_error: last_error.clone(),
            },
            RuntimeSignal::Geoblock {
                status,
                last_observed_at,
                last_error,
            } => CapabilityHealth {
                capability: "geoblock".into(),
                required_for_submit: true,
                level: match status {
                    GeoblockStatus::Allowed => HealthLevel::Healthy,
                    GeoblockStatus::Blocked => HealthLevel::Degraded,
                    GeoblockStatus::Unknown => HealthLevel::Unknown,
                    GeoblockStatus::Error => HealthLevel::Degraded,
                },
                last_observed_at: *last_observed_at,
                last_error: last_error.clone(),
            },
            RuntimeSignal::ResourceRefresh {
                fresh,
                last_observed_at,
                last_error,
            } => CapabilityHealth {
                capability: "resource-refresh".into(),
                required_for_submit: true,
                level: if *fresh {
                    HealthLevel::Healthy
                } else {
                    HealthLevel::Stale
                },
                last_observed_at: *last_observed_at,
                last_error: last_error.clone(),
            },
            RuntimeSignal::ReconcileBacklog {
                remote_unknown_orders,
                last_observed_at,
                last_error,
            } => CapabilityHealth {
                capability: "reconcile-backlog".into(),
                required_for_submit: true,
                level: if *remote_unknown_orders == 0 {
                    HealthLevel::Healthy
                } else {
                    HealthLevel::Degraded
                },
                last_observed_at: *last_observed_at,
                last_error: last_error.clone(),
            },
        }
    }

    pub(crate) fn worker_kind(&self) -> RuntimeWorkerKind {
        match self {
            RuntimeSignal::WebSocket { .. } => RuntimeWorkerKind::WebSocketLiveness,
            RuntimeSignal::HeartbeatLease { .. } => RuntimeWorkerKind::HeartbeatLease,
            RuntimeSignal::Geoblock { .. } => RuntimeWorkerKind::Geoblock,
            RuntimeSignal::ResourceRefresh { .. } => RuntimeWorkerKind::ResourceRefresh,
            RuntimeSignal::ReconcileBacklog { .. } => RuntimeWorkerKind::ReconcileBacklog,
        }
    }
}

pub fn runtime_breakdown_from_signals(
    account_id: impl Into<String>,
    signals: &[RuntimeSignal],
) -> RuntimeHealthBreakdown {
    let mut account_capabilities = Vec::new();
    let mut market_capabilities = Vec::new();
    let asset_capabilities = Vec::new();
    let mut worker_capabilities = Vec::new();

    for signal in signals {
        let health = signal.to_capability_health();
        match signal {
            RuntimeSignal::Geoblock { .. } | RuntimeSignal::HeartbeatLease { .. } => {
                account_capabilities.push(health);
            }
            RuntimeSignal::WebSocket { .. } => market_capabilities.push(health),
            RuntimeSignal::ResourceRefresh { .. } | RuntimeSignal::ReconcileBacklog { .. } => {
                worker_capabilities.push(health);
            }
        }
    }

    RuntimeHealthBreakdown {
        account_id: account_id.into(),
        account_capabilities,
        market_capabilities,
        asset_capabilities,
        worker_capabilities,
    }
}
