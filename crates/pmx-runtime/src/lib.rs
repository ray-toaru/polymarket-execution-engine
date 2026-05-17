use chrono::{DateTime, Utc};
use pmx_core::GeoblockStatus;
use serde::{Deserialize, Serialize};
use tokio::time::{Duration, interval};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerRole {
    Heartbeat,
    MarketWebSocket,
    UserWebSocket,
    SportsWebSocket,
    ResourceRefresh,
    Reconcile,
    ReservationSweeper,
    ReleaseGuard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerHeartbeat {
    pub worker_id: String,
    pub role: WorkerRole,
    pub capability: String,
    pub observed_at: DateTime<Utc>,
    pub last_error: Option<String>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WebSocketChannel {
    Market,
    User,
    Sports,
}

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
            RuntimeSignal::ReconcileBacklog { .. } => worker_capabilities.push(health),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RuntimeWorkerKind {
    WebSocketLiveness,
    HeartbeatLease,
    Geoblock,
    ReconcileBacklog,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeWorkerAction {
    pub kind: RuntimeWorkerKind,
    pub capability: String,
    pub should_fail_closed: bool,
    pub should_update_runtime_store: bool,
    pub reason: String,
}

pub fn worker_actions_from_runtime_signals(signals: &[RuntimeSignal]) -> Vec<RuntimeWorkerAction> {
    signals
        .iter()
        .map(|signal| {
            let health = signal.to_capability_health();
            let kind = match signal {
                RuntimeSignal::WebSocket { .. } => RuntimeWorkerKind::WebSocketLiveness,
                RuntimeSignal::HeartbeatLease { .. } => RuntimeWorkerKind::HeartbeatLease,
                RuntimeSignal::Geoblock { .. } => RuntimeWorkerKind::Geoblock,
                RuntimeSignal::ReconcileBacklog { .. } => RuntimeWorkerKind::ReconcileBacklog,
            };
            RuntimeWorkerAction {
                kind,
                capability: health.capability.clone(),
                should_fail_closed: health.blocks_submit(),
                should_update_runtime_store: true,
                reason: health
                    .last_error
                    .clone()
                    .unwrap_or_else(|| format!("{:?}", health.level)),
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeWorkerStoreWrite {
    pub account_id: String,
    pub capability: String,
    pub worker_kind: RuntimeWorkerKind,
    pub status: HealthLevel,
    pub should_fail_closed: bool,
    pub reason: String,
}

/// Prepare deterministic store-write payloads for runtime worker observations.
///
/// This helper deliberately does not talk to PostgreSQL. Store crates can map the returned
/// payload into `runtime_worker_observations` or capability-specific truth tables after
/// applying their own transaction and idempotency policy.
pub fn runtime_worker_store_writes(
    account_id: impl Into<String>,
    signals: &[RuntimeSignal],
) -> Vec<RuntimeWorkerStoreWrite> {
    let account_id = account_id.into();
    signals
        .iter()
        .map(|signal| {
            let health = signal.to_capability_health();
            let kind = match signal {
                RuntimeSignal::WebSocket { .. } => RuntimeWorkerKind::WebSocketLiveness,
                RuntimeSignal::HeartbeatLease { .. } => RuntimeWorkerKind::HeartbeatLease,
                RuntimeSignal::Geoblock { .. } => RuntimeWorkerKind::Geoblock,
                RuntimeSignal::ReconcileBacklog { .. } => RuntimeWorkerKind::ReconcileBacklog,
            };
            let status = health.level.clone();
            let reason = health
                .last_error
                .clone()
                .unwrap_or_else(|| format!("{:?}", status));
            let should_fail_closed = health.blocks_submit();
            RuntimeWorkerStoreWrite {
                account_id: account_id.clone(),
                capability: health.capability,
                worker_kind: kind,
                status,
                should_fail_closed,
                reason,
            }
        })
        .collect()
}

pub async fn run_placeholder_worker(worker_id: String) {
    let mut ticker = interval(Duration::from_secs(30));
    loop {
        ticker.tick().await;
        let _heartbeat = WorkerHeartbeat {
            worker_id: worker_id.clone(),
            role: WorkerRole::Heartbeat,
            capability: "heartbeat".to_string(),
            observed_at: Utc::now(),
            last_error: None,
        };
        // v0.1 placeholder. Real implementation persists heartbeat to worker_health.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocking_capabilities_include_submit_required_stale_worker() {
        let breakdown = RuntimeHealthBreakdown {
            account_id: "acct-1".into(),
            account_capabilities: vec![],
            market_capabilities: vec![],
            asset_capabilities: vec![],
            worker_capabilities: vec![CapabilityHealth {
                capability: "heartbeat".into(),
                required_for_submit: true,
                level: HealthLevel::Stale,
                last_observed_at: None,
                last_error: Some("missed heartbeat".into()),
            }],
        };
        assert_eq!(breakdown.blocking_capabilities().len(), 1);
    }

    #[test]
    fn runtime_signals_map_websocket_and_heartbeat_to_submit_blockers() {
        let breakdown = runtime_breakdown_from_signals(
            "acct-runtime-v20",
            &[
                RuntimeSignal::WebSocket {
                    channel: WebSocketChannel::Market,
                    connected: true,
                    stale: true,
                    last_observed_at: None,
                    last_error: Some("market stream stale".into()),
                },
                RuntimeSignal::HeartbeatLease {
                    active: false,
                    last_observed_at: None,
                    last_error: Some("lease expired".into()),
                },
            ],
        );
        let blockers = breakdown.blocking_capabilities();
        assert_eq!(blockers.len(), 2);
        assert!(blockers.iter().any(|h| h.capability == "websocket:market"));
        assert!(blockers.iter().any(|h| h.capability == "heartbeat-lease"));
    }

    #[test]
    fn geoblock_unknown_and_reconcile_backlog_block_submit() {
        let breakdown = runtime_breakdown_from_signals(
            "acct-runtime-v20",
            &[
                RuntimeSignal::Geoblock {
                    status: GeoblockStatus::Unknown,
                    last_observed_at: None,
                    last_error: Some("provider timeout".into()),
                },
                RuntimeSignal::ReconcileBacklog {
                    remote_unknown_orders: 1,
                    last_observed_at: None,
                    last_error: None,
                },
            ],
        );
        let names: Vec<_> = breakdown
            .blocking_capabilities()
            .into_iter()
            .map(|h| h.capability.as_str())
            .collect();
        assert_eq!(names, vec!["geoblock", "reconcile-backlog"]);
    }

    #[test]
    fn worker_actions_mark_stale_runtime_inputs_as_fail_closed_updates() {
        let actions = worker_actions_from_runtime_signals(&[
            RuntimeSignal::HeartbeatLease {
                active: false,
                last_observed_at: None,
                last_error: Some("lease expired".into()),
            },
            RuntimeSignal::WebSocket {
                channel: WebSocketChannel::User,
                connected: false,
                stale: true,
                last_observed_at: None,
                last_error: Some("user stream disconnected".into()),
            },
        ]);
        assert_eq!(actions.len(), 2);
        assert!(actions.iter().all(|action| action.should_fail_closed));
        assert!(
            actions
                .iter()
                .all(|action| action.should_update_runtime_store)
        );
        assert!(
            actions
                .iter()
                .any(|action| action.capability == "heartbeat-lease")
        );
        assert!(
            actions
                .iter()
                .any(|action| action.capability == "websocket:user")
        );
    }
}

#[cfg(test)]
mod capability_tests_v07 {
    use super::*;

    #[test]
    fn degraded_non_required_capability_does_not_block_submit() {
        let health = CapabilityHealth {
            capability: "reservation-sweeper".into(),
            required_for_submit: false,
            level: HealthLevel::Degraded,
            last_observed_at: None,
            last_error: Some("behind schedule".into()),
        };
        assert!(!health.blocks_submit());
    }

    #[test]
    fn all_capabilities_preserves_account_market_asset_worker_groups() {
        let mk = |capability: &str| CapabilityHealth {
            capability: capability.into(),
            required_for_submit: true,
            level: HealthLevel::Healthy,
            last_observed_at: None,
            last_error: None,
        };
        let breakdown = RuntimeHealthBreakdown {
            account_id: "acct-runtime-v07".into(),
            account_capabilities: vec![mk("account-runtime")],
            market_capabilities: vec![mk("market-ws")],
            asset_capabilities: vec![mk("collateral-profile")],
            worker_capabilities: vec![mk("heartbeat-worker")],
        };
        let names: Vec<_> = breakdown
            .all_capabilities()
            .into_iter()
            .map(|health| health.capability.as_str())
            .collect();
        assert_eq!(
            names,
            vec![
                "account-runtime",
                "market-ws",
                "collateral-profile",
                "heartbeat-worker"
            ]
        );
    }

    #[test]
    fn runtime_worker_store_writes_are_fail_closed_for_bad_signals() {
        let writes = runtime_worker_store_writes(
            "acct-worker-write",
            &[
                RuntimeSignal::Geoblock {
                    status: GeoblockStatus::Unknown,
                    last_observed_at: None,
                    last_error: Some("geoblock unavailable".into()),
                },
                RuntimeSignal::HeartbeatLease {
                    active: false,
                    last_observed_at: None,
                    last_error: Some("heartbeat lease expired".into()),
                },
            ],
        );
        assert_eq!(writes.len(), 2);
        assert!(writes.iter().all(|write| write.should_fail_closed));
        assert!(writes.iter().any(|write| write.capability == "geoblock"));
        assert!(
            writes
                .iter()
                .any(|write| write.capability == "heartbeat-lease")
        );
    }
}
