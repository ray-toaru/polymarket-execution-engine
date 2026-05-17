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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RuntimeWorkerKind {
    WebSocketLiveness,
    HeartbeatLease,
    Geoblock,
    ResourceRefresh,
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
                RuntimeSignal::ResourceRefresh { .. } => RuntimeWorkerKind::ResourceRefresh,
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
                RuntimeSignal::ResourceRefresh { .. } => RuntimeWorkerKind::ResourceRefresh,
                RuntimeSignal::ReconcileBacklog { .. } => RuntimeWorkerKind::ReconcileBacklog,
            };
            let status = health.level.clone();
            let reason = health
                .last_error
                .clone()
                .unwrap_or_else(|| format!("{status:?}"));
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeWorkerLoopInput {
    pub account_id: String,
    pub lease_owner_id: String,
    pub instance_id: String,
    pub market_websocket_connected: bool,
    pub market_websocket_stale: bool,
    pub user_websocket_connected: bool,
    pub user_websocket_stale: bool,
    pub geoblock_status: GeoblockStatus,
    pub resource_refresh_fresh: bool,
    pub remote_unknown_orders: u32,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeWorkerProviderSnapshot {
    pub account_id: String,
    pub lease_owner_id: String,
    pub instance_id: String,
    pub market_websocket_connected: bool,
    pub market_websocket_stale: bool,
    pub user_websocket_connected: bool,
    pub user_websocket_stale: bool,
    pub geoblock_status: GeoblockStatus,
    pub resource_refresh_fresh: bool,
    pub remote_unknown_orders: u32,
    pub observed_at: DateTime<Utc>,
    pub provider_name: String,
    pub no_trading_side_effect: bool,
}

impl RuntimeWorkerProviderSnapshot {
    pub fn into_loop_input(self) -> RuntimeWorkerLoopInput {
        RuntimeWorkerLoopInput {
            account_id: self.account_id,
            lease_owner_id: self.lease_owner_id,
            instance_id: self.instance_id,
            market_websocket_connected: self.market_websocket_connected,
            market_websocket_stale: self.market_websocket_stale,
            user_websocket_connected: self.user_websocket_connected,
            user_websocket_stale: self.user_websocket_stale,
            geoblock_status: self.geoblock_status,
            resource_refresh_fresh: self.resource_refresh_fresh,
            remote_unknown_orders: self.remote_unknown_orders,
            observed_at: self.observed_at,
        }
    }
}

pub trait RuntimeWorkerProvider {
    fn snapshot(&self) -> RuntimeWorkerProviderSnapshot;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeartbeatLeaseCandidate {
    pub worker_id: String,
    pub status: HealthLevel,
    pub last_heartbeat_at: DateTime<Utc>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeartbeatLeaseElectionInput {
    pub instance_id: String,
    pub candidates: Vec<HeartbeatLeaseCandidate>,
    pub observed_at: DateTime<Utc>,
    pub stale_after_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeartbeatLeaseElection {
    pub lease_owner_id: String,
    pub lease_owner_active: bool,
    pub healthy_candidate_count: usize,
    pub fail_closed: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResourceRefreshComponent {
    Account,
    Market,
    Collateral,
}

impl ResourceRefreshComponent {
    fn as_str(&self) -> &'static str {
        match self {
            ResourceRefreshComponent::Account => "account",
            ResourceRefreshComponent::Market => "market",
            ResourceRefreshComponent::Collateral => "collateral",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceRefreshObservation {
    pub component: ResourceRefreshComponent,
    pub resource_id: String,
    pub refreshed_at: DateTime<Utc>,
    pub status: HealthLevel,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceRefreshEvaluationInput {
    pub observations: Vec<ResourceRefreshObservation>,
    pub observed_at: DateTime<Utc>,
    pub stale_after_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceRefreshEvaluation {
    pub fresh: bool,
    pub stale_components: Vec<String>,
    pub failed_components: Vec<String>,
    pub missing_components: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReconcileBacklogEvaluationInput {
    pub remote_unknown_order_ids: Vec<String>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReconcileBacklogEvaluation {
    pub remote_unknown_orders: u32,
    pub submit_blocked: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebSocketLivenessObservation {
    pub channel: WebSocketChannel,
    pub connected: bool,
    pub last_message_at: Option<DateTime<Utc>>,
    pub status: HealthLevel,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebSocketLivenessEvaluationInput {
    pub observations: Vec<WebSocketLivenessObservation>,
    pub observed_at: DateTime<Utc>,
    pub stale_after_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebSocketLivenessEvaluation {
    pub market_connected: bool,
    pub market_stale: bool,
    pub user_connected: bool,
    pub user_stale: bool,
    pub missing_channels: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeoblockEvaluationInput {
    pub status: GeoblockStatus,
    pub observed_at: DateTime<Utc>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeoblockEvaluation {
    pub status: GeoblockStatus,
    pub submit_allowed: bool,
    pub reason: String,
}

/// Evaluate resource-refresh freshness without doing network or store I/O.
///
/// Every observed resource must be fresh and healthy. Missing observations are
/// fail-closed because a submit decision cannot prove account, market, and
/// collateral resources are current.
pub fn evaluate_resource_refresh_freshness(
    input: ResourceRefreshEvaluationInput,
) -> ResourceRefreshEvaluation {
    if input.observations.is_empty() {
        return ResourceRefreshEvaluation {
            fresh: false,
            stale_components: vec![],
            failed_components: vec![],
            missing_components: vec!["account".into(), "market".into(), "collateral".into()],
            reason: "no resource refresh observations".into(),
        };
    }

    let stale_after_seconds = input.stale_after_seconds.max(0);
    let cutoff = input.observed_at - chrono::Duration::seconds(stale_after_seconds);
    let mut stale_components = Vec::new();
    let mut failed_components = Vec::new();
    let mut has_account = false;
    let mut has_market = false;
    let mut has_collateral = false;
    for observation in input.observations {
        match observation.component {
            ResourceRefreshComponent::Account => has_account = true,
            ResourceRefreshComponent::Market => has_market = true,
            ResourceRefreshComponent::Collateral => has_collateral = true,
        }
        let component = format!(
            "{}:{}",
            observation.component.as_str(),
            observation.resource_id
        );
        if observation.status != HealthLevel::Healthy {
            failed_components.push(component);
        } else if observation.refreshed_at < cutoff {
            stale_components.push(component);
        }
    }

    let mut missing_components = Vec::new();
    if !has_account {
        missing_components.push("account".into());
    }
    if !has_market {
        missing_components.push("market".into());
    }
    if !has_collateral {
        missing_components.push("collateral".into());
    }

    let fresh = stale_components.is_empty()
        && failed_components.is_empty()
        && missing_components.is_empty();
    let reason = if fresh {
        "all resource refresh observations are fresh".into()
    } else {
        format!(
            "stale_components={} failed_components={} missing_components={}",
            stale_components.len(),
            failed_components.len(),
            missing_components.len()
        )
    };
    ResourceRefreshEvaluation {
        fresh,
        stale_components,
        failed_components,
        missing_components,
        reason,
    }
}

/// Evaluate WebSocket liveness for submit-critical market and user channels.
pub fn evaluate_websocket_liveness(
    input: WebSocketLivenessEvaluationInput,
) -> WebSocketLivenessEvaluation {
    let stale_after_seconds = input.stale_after_seconds.max(0);
    let cutoff = input.observed_at - chrono::Duration::seconds(stale_after_seconds);
    let mut market = None;
    let mut user = None;

    for observation in input.observations {
        match observation.channel {
            WebSocketChannel::Market => market = Some(observation),
            WebSocketChannel::User => user = Some(observation),
            WebSocketChannel::Sports => {}
        }
    }

    let mut missing_channels = Vec::new();
    let (market_connected, market_stale) = match market {
        Some(observation) => websocket_observation_state(&observation, cutoff),
        None => {
            missing_channels.push("market".into());
            (false, true)
        }
    };
    let (user_connected, user_stale) = match user {
        Some(observation) => websocket_observation_state(&observation, cutoff),
        None => {
            missing_channels.push("user".into());
            (false, true)
        }
    };
    let healthy = market_connected && !market_stale && user_connected && !user_stale;
    WebSocketLivenessEvaluation {
        market_connected,
        market_stale,
        user_connected,
        user_stale,
        missing_channels,
        reason: if healthy {
            "market and user websocket channels are live".into()
        } else {
            "market or user websocket channel is disconnected, stale, or missing".into()
        },
    }
}

fn websocket_observation_state(
    observation: &WebSocketLivenessObservation,
    cutoff: DateTime<Utc>,
) -> (bool, bool) {
    let connected = observation.connected && observation.status == HealthLevel::Healthy;
    let stale = observation
        .last_message_at
        .map(|last_message_at| last_message_at < cutoff)
        .unwrap_or(true);
    (connected, stale)
}

/// Evaluate geoblock provider status without remote I/O.
pub fn evaluate_geoblock_status(input: GeoblockEvaluationInput) -> GeoblockEvaluation {
    let submit_allowed = matches!(input.status, GeoblockStatus::Allowed);
    GeoblockEvaluation {
        status: input.status,
        submit_allowed,
        reason: if submit_allowed {
            "geoblock provider allowed".into()
        } else {
            input
                .last_error
                .unwrap_or_else(|| "geoblock provider did not allow submit".into())
        },
    }
}

/// Evaluate reconcile backlog without reading or mutating remote order state.
pub fn evaluate_reconcile_backlog(
    input: ReconcileBacklogEvaluationInput,
) -> ReconcileBacklogEvaluation {
    let remote_unknown_orders = input.remote_unknown_order_ids.len() as u32;
    ReconcileBacklogEvaluation {
        remote_unknown_orders,
        submit_blocked: remote_unknown_orders > 0,
        reason: if remote_unknown_orders == 0 {
            "no remote unknown reconcile backlog".into()
        } else {
            format!("remote_unknown_orders={remote_unknown_orders}")
        },
    }
}

/// Elect a single heartbeat lease owner from local worker health observations.
///
/// Election is deterministic: only fresh `Healthy` candidates are eligible, the
/// freshest heartbeat wins, and worker_id breaks ties. Absence of a fresh owner
/// fails closed; it must not produce an allow-like runtime state.
pub fn elect_heartbeat_lease_owner(input: HeartbeatLeaseElectionInput) -> HeartbeatLeaseElection {
    let stale_after_seconds = input.stale_after_seconds.max(0);
    let cutoff = input.observed_at - chrono::Duration::seconds(stale_after_seconds);
    let mut healthy: Vec<_> = input
        .candidates
        .into_iter()
        .filter(|candidate| {
            candidate.status == HealthLevel::Healthy && candidate.last_heartbeat_at >= cutoff
        })
        .collect();
    healthy.sort_by(|left, right| {
        right
            .last_heartbeat_at
            .cmp(&left.last_heartbeat_at)
            .then_with(|| left.worker_id.cmp(&right.worker_id))
    });
    let healthy_candidate_count = healthy.len();
    let Some(owner) = healthy.first() else {
        return HeartbeatLeaseElection {
            lease_owner_id: String::new(),
            lease_owner_active: false,
            healthy_candidate_count,
            fail_closed: true,
            reason: "no fresh healthy heartbeat lease candidate".into(),
        };
    };
    let lease_owner_active = owner.worker_id == input.instance_id;
    HeartbeatLeaseElection {
        lease_owner_id: owner.worker_id.clone(),
        lease_owner_active,
        healthy_candidate_count,
        fail_closed: !lease_owner_active,
        reason: if lease_owner_active {
            "local instance owns heartbeat lease".into()
        } else {
            "another fresh heartbeat lease owner is active".into()
        },
    }
}

pub fn runtime_worker_loop_tick_from_provider<P: RuntimeWorkerProvider>(
    provider: &P,
) -> RuntimeWorkerLoopTick {
    let snapshot = provider.snapshot();
    assert!(
        snapshot.no_trading_side_effect,
        "runtime worker providers must not trade"
    );
    runtime_worker_loop_tick(snapshot.into_loop_input())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeWorkerLoopTick {
    pub account_id: String,
    pub lease_owner_active: bool,
    pub signals: Vec<RuntimeSignal>,
    pub actions: Vec<RuntimeWorkerAction>,
    pub submit_allowed_by_runtime: bool,
}

/// Build one deterministic runtime worker tick from observed worker inputs.
///
/// Network workers and store crates own I/O. This function is the pure boundary
/// that makes disconnects, stale leases, geoblocks, stale resource refreshes,
/// and reconcile backlog consistently fail closed before submit decisions.
pub fn runtime_worker_loop_tick(input: RuntimeWorkerLoopInput) -> RuntimeWorkerLoopTick {
    let lease_owner_active = input.lease_owner_id == input.instance_id;
    let observed_at = Some(input.observed_at);
    let geoblock_allowed = matches!(input.geoblock_status, GeoblockStatus::Allowed);
    let signals = vec![
        RuntimeSignal::HeartbeatLease {
            active: lease_owner_active,
            last_observed_at: observed_at,
            last_error: (!lease_owner_active).then(|| "stale lease owner".into()),
        },
        RuntimeSignal::WebSocket {
            channel: WebSocketChannel::Market,
            connected: input.market_websocket_connected,
            stale: input.market_websocket_stale,
            last_observed_at: observed_at,
            last_error: (!input.market_websocket_connected || input.market_websocket_stale)
                .then(|| "market websocket unhealthy".into()),
        },
        RuntimeSignal::WebSocket {
            channel: WebSocketChannel::User,
            connected: input.user_websocket_connected,
            stale: input.user_websocket_stale,
            last_observed_at: observed_at,
            last_error: (!input.user_websocket_connected || input.user_websocket_stale)
                .then(|| "user websocket unhealthy".into()),
        },
        RuntimeSignal::Geoblock {
            status: input.geoblock_status,
            last_observed_at: observed_at,
            last_error: (!geoblock_allowed).then(|| "geoblock not allowed".into()),
        },
        RuntimeSignal::ResourceRefresh {
            fresh: input.resource_refresh_fresh,
            last_observed_at: observed_at,
            last_error: (!input.resource_refresh_fresh).then(|| "resource refresh stale".into()),
        },
        RuntimeSignal::ReconcileBacklog {
            remote_unknown_orders: input.remote_unknown_orders,
            last_observed_at: observed_at,
            last_error: (input.remote_unknown_orders > 0).then(|| "remote unknown backlog".into()),
        },
    ];
    let actions = worker_actions_from_runtime_signals(&signals);
    let submit_allowed_by_runtime = actions.iter().all(|action| !action.should_fail_closed);
    RuntimeWorkerLoopTick {
        account_id: input.account_id,
        lease_owner_active,
        signals,
        actions,
        submit_allowed_by_runtime,
    }
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
                RuntimeSignal::ResourceRefresh {
                    fresh: false,
                    last_observed_at: None,
                    last_error: Some("resource refresh stale".into()),
                },
            ],
        );
        let names: Vec<_> = breakdown
            .blocking_capabilities()
            .into_iter()
            .map(|h| h.capability.as_str())
            .collect();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"geoblock"));
        assert!(names.contains(&"resource-refresh"));
        assert!(names.contains(&"reconcile-backlog"));
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
                RuntimeSignal::ResourceRefresh {
                    fresh: false,
                    last_observed_at: None,
                    last_error: Some("resource refresh stale".into()),
                },
            ],
        );
        assert_eq!(writes.len(), 3);
        assert!(writes.iter().all(|write| write.should_fail_closed));
        assert!(writes.iter().any(|write| write.capability == "geoblock"));
        assert!(
            writes
                .iter()
                .any(|write| write.capability == "heartbeat-lease")
        );
        assert!(
            writes
                .iter()
                .any(|write| write.capability == "resource-refresh")
        );
    }

    #[test]
    fn runtime_worker_loop_tick_blocks_stale_down_and_geoblocked_submit() {
        let tick = runtime_worker_loop_tick(RuntimeWorkerLoopInput {
            account_id: "acct-runtime-loop".into(),
            lease_owner_id: "worker-a".into(),
            instance_id: "worker-b".into(),
            market_websocket_connected: false,
            market_websocket_stale: true,
            user_websocket_connected: true,
            user_websocket_stale: true,
            geoblock_status: GeoblockStatus::Blocked,
            resource_refresh_fresh: false,
            remote_unknown_orders: 2,
            observed_at: Utc::now(),
        });
        assert!(!tick.lease_owner_active);
        assert!(!tick.submit_allowed_by_runtime);
        assert_eq!(tick.signals.len(), 6);
        assert!(
            tick.actions
                .iter()
                .any(|action| action.capability == "heartbeat-lease" && action.should_fail_closed)
        );
        assert!(
            tick.actions
                .iter()
                .any(|action| action.capability == "websocket:market" && action.should_fail_closed)
        );
        assert!(
            tick.actions
                .iter()
                .any(|action| action.capability == "geoblock" && action.should_fail_closed)
        );
        assert!(
            tick.actions
                .iter()
                .any(|action| action.capability == "resource-refresh" && action.should_fail_closed)
        );
        assert!(
            tick.actions
                .iter()
                .any(|action| action.capability == "reconcile-backlog" && action.should_fail_closed)
        );
    }

    #[test]
    fn runtime_worker_loop_tick_recovers_only_after_all_required_inputs_are_healthy() {
        let tick = runtime_worker_loop_tick(RuntimeWorkerLoopInput {
            account_id: "acct-runtime-loop".into(),
            lease_owner_id: "worker-a".into(),
            instance_id: "worker-a".into(),
            market_websocket_connected: true,
            market_websocket_stale: false,
            user_websocket_connected: true,
            user_websocket_stale: false,
            geoblock_status: GeoblockStatus::Allowed,
            resource_refresh_fresh: true,
            remote_unknown_orders: 0,
            observed_at: Utc::now(),
        });
        assert!(tick.lease_owner_active);
        assert!(tick.submit_allowed_by_runtime);
        assert!(tick.actions.iter().all(|action| !action.should_fail_closed));
    }

    struct FakeRuntimeWorkerProvider(RuntimeWorkerProviderSnapshot);

    impl RuntimeWorkerProvider for FakeRuntimeWorkerProvider {
        fn snapshot(&self) -> RuntimeWorkerProviderSnapshot {
            self.0.clone()
        }
    }

    #[test]
    fn runtime_worker_provider_snapshot_feeds_loop_without_trading_side_effects() {
        let provider = FakeRuntimeWorkerProvider(RuntimeWorkerProviderSnapshot {
            account_id: "acct-provider".into(),
            lease_owner_id: "worker-1".into(),
            instance_id: "worker-1".into(),
            market_websocket_connected: true,
            market_websocket_stale: false,
            user_websocket_connected: false,
            user_websocket_stale: true,
            geoblock_status: GeoblockStatus::Allowed,
            resource_refresh_fresh: true,
            remote_unknown_orders: 0,
            observed_at: Utc::now(),
            provider_name: "fake-provider".into(),
            no_trading_side_effect: true,
        });
        let tick = runtime_worker_loop_tick_from_provider(&provider);
        assert!(!tick.submit_allowed_by_runtime);
        assert!(
            tick.actions
                .iter()
                .any(|action| action.capability == "websocket:user" && action.should_fail_closed)
        );
    }

    #[test]
    fn heartbeat_lease_election_selects_fresh_owner_and_fails_closed_for_non_owner() {
        let observed_at = Utc::now();
        let election = elect_heartbeat_lease_owner(HeartbeatLeaseElectionInput {
            instance_id: "worker-b".into(),
            observed_at,
            stale_after_seconds: 30,
            candidates: vec![
                HeartbeatLeaseCandidate {
                    worker_id: "worker-a".into(),
                    status: HealthLevel::Healthy,
                    last_heartbeat_at: observed_at - chrono::Duration::seconds(5),
                    last_error: None,
                },
                HeartbeatLeaseCandidate {
                    worker_id: "worker-b".into(),
                    status: HealthLevel::Healthy,
                    last_heartbeat_at: observed_at - chrono::Duration::seconds(10),
                    last_error: None,
                },
            ],
        });
        assert_eq!(election.lease_owner_id, "worker-a");
        assert!(!election.lease_owner_active);
        assert!(election.fail_closed);
    }

    #[test]
    fn heartbeat_lease_election_has_no_owner_when_all_candidates_are_stale() {
        let observed_at = Utc::now();
        let election = elect_heartbeat_lease_owner(HeartbeatLeaseElectionInput {
            instance_id: "worker-a".into(),
            observed_at,
            stale_after_seconds: 30,
            candidates: vec![HeartbeatLeaseCandidate {
                worker_id: "worker-a".into(),
                status: HealthLevel::Healthy,
                last_heartbeat_at: observed_at - chrono::Duration::seconds(60),
                last_error: Some("missed heartbeat".into()),
            }],
        });
        assert!(election.lease_owner_id.is_empty());
        assert!(election.fail_closed);
        assert_eq!(election.healthy_candidate_count, 0);
    }

    #[test]
    fn resource_refresh_evaluation_accepts_fresh_healthy_observations() {
        let observed_at = Utc::now();
        let evaluation = evaluate_resource_refresh_freshness(ResourceRefreshEvaluationInput {
            observed_at,
            stale_after_seconds: 30,
            observations: vec![
                ResourceRefreshObservation {
                    component: ResourceRefreshComponent::Account,
                    resource_id: "acct-1".into(),
                    refreshed_at: observed_at - chrono::Duration::seconds(5),
                    status: HealthLevel::Healthy,
                    last_error: None,
                },
                ResourceRefreshObservation {
                    component: ResourceRefreshComponent::Market,
                    resource_id: "cond-1".into(),
                    refreshed_at: observed_at - chrono::Duration::seconds(10),
                    status: HealthLevel::Healthy,
                    last_error: None,
                },
                ResourceRefreshObservation {
                    component: ResourceRefreshComponent::Collateral,
                    resource_id: "collateral-1".into(),
                    refreshed_at: observed_at - chrono::Duration::seconds(15),
                    status: HealthLevel::Healthy,
                    last_error: None,
                },
            ],
        });
        assert!(evaluation.fresh);
        assert!(evaluation.stale_components.is_empty());
        assert!(evaluation.failed_components.is_empty());
        assert!(evaluation.missing_components.is_empty());
    }

    #[test]
    fn resource_refresh_evaluation_fails_closed_for_stale_failed_or_missing_inputs() {
        let observed_at = Utc::now();
        let missing = evaluate_resource_refresh_freshness(ResourceRefreshEvaluationInput {
            observed_at,
            stale_after_seconds: 30,
            observations: vec![],
        });
        assert!(!missing.fresh);
        assert_eq!(missing.reason, "no resource refresh observations");
        assert_eq!(
            missing.missing_components,
            vec!["account", "market", "collateral"]
        );

        let evaluation = evaluate_resource_refresh_freshness(ResourceRefreshEvaluationInput {
            observed_at,
            stale_after_seconds: 30,
            observations: vec![
                ResourceRefreshObservation {
                    component: ResourceRefreshComponent::Account,
                    resource_id: "acct-1".into(),
                    refreshed_at: observed_at - chrono::Duration::seconds(31),
                    status: HealthLevel::Healthy,
                    last_error: None,
                },
                ResourceRefreshObservation {
                    component: ResourceRefreshComponent::Collateral,
                    resource_id: "collateral-1".into(),
                    refreshed_at: observed_at - chrono::Duration::seconds(1),
                    status: HealthLevel::Degraded,
                    last_error: Some("balance refresh failed".into()),
                },
            ],
        });
        assert!(!evaluation.fresh);
        assert_eq!(evaluation.stale_components, vec!["account:acct-1"]);
        assert_eq!(
            evaluation.failed_components,
            vec!["collateral:collateral-1"]
        );
        assert_eq!(evaluation.missing_components, vec!["market"]);
    }

    #[test]
    fn reconcile_backlog_evaluation_blocks_submit_for_remote_unknown_orders() {
        let evaluation = evaluate_reconcile_backlog(ReconcileBacklogEvaluationInput {
            remote_unknown_order_ids: vec!["order-1".into(), "order-2".into()],
            observed_at: Utc::now(),
        });
        assert_eq!(evaluation.remote_unknown_orders, 2);
        assert!(evaluation.submit_blocked);
        assert_eq!(evaluation.reason, "remote_unknown_orders=2");
    }

    #[test]
    fn reconcile_backlog_evaluation_allows_submit_when_backlog_empty() {
        let evaluation = evaluate_reconcile_backlog(ReconcileBacklogEvaluationInput {
            remote_unknown_order_ids: vec![],
            observed_at: Utc::now(),
        });
        assert_eq!(evaluation.remote_unknown_orders, 0);
        assert!(!evaluation.submit_blocked);
        assert_eq!(evaluation.reason, "no remote unknown reconcile backlog");
    }

    #[test]
    fn websocket_liveness_evaluation_accepts_fresh_market_and_user_channels() {
        let observed_at = Utc::now();
        let evaluation = evaluate_websocket_liveness(WebSocketLivenessEvaluationInput {
            observed_at,
            stale_after_seconds: 30,
            observations: vec![
                WebSocketLivenessObservation {
                    channel: WebSocketChannel::Market,
                    connected: true,
                    last_message_at: Some(observed_at - chrono::Duration::seconds(5)),
                    status: HealthLevel::Healthy,
                    last_error: None,
                },
                WebSocketLivenessObservation {
                    channel: WebSocketChannel::User,
                    connected: true,
                    last_message_at: Some(observed_at - chrono::Duration::seconds(10)),
                    status: HealthLevel::Healthy,
                    last_error: None,
                },
            ],
        });
        assert!(evaluation.market_connected);
        assert!(!evaluation.market_stale);
        assert!(evaluation.user_connected);
        assert!(!evaluation.user_stale);
        assert!(evaluation.missing_channels.is_empty());
    }

    #[test]
    fn websocket_liveness_evaluation_fails_closed_for_disconnected_stale_or_missing_channels() {
        let observed_at = Utc::now();
        let evaluation = evaluate_websocket_liveness(WebSocketLivenessEvaluationInput {
            observed_at,
            stale_after_seconds: 30,
            observations: vec![WebSocketLivenessObservation {
                channel: WebSocketChannel::Market,
                connected: true,
                last_message_at: Some(observed_at - chrono::Duration::seconds(31)),
                status: HealthLevel::Healthy,
                last_error: None,
            }],
        });
        assert!(evaluation.market_connected);
        assert!(evaluation.market_stale);
        assert!(!evaluation.user_connected);
        assert!(evaluation.user_stale);
        assert_eq!(evaluation.missing_channels, vec!["user"]);
    }

    #[test]
    fn geoblock_evaluation_only_allows_explicit_allowed_status() {
        let allowed = evaluate_geoblock_status(GeoblockEvaluationInput {
            status: GeoblockStatus::Allowed,
            observed_at: Utc::now(),
            last_error: None,
        });
        assert!(allowed.submit_allowed);

        let blocked = evaluate_geoblock_status(GeoblockEvaluationInput {
            status: GeoblockStatus::Unknown,
            observed_at: Utc::now(),
            last_error: Some("provider timeout".into()),
        });
        assert!(!blocked.submit_allowed);
        assert_eq!(blocked.reason, "provider timeout");
    }
}
