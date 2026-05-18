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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkerCrashRecoveryObservation {
    pub worker_id: String,
    pub capability: String,
    pub status: HealthLevel,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkerCrashRecoveryEvaluationInput {
    pub observations: Vec<WorkerCrashRecoveryObservation>,
    pub required_capabilities: Vec<String>,
    pub observed_at: DateTime<Utc>,
    pub stale_after_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkerCrashRecoveryEvaluation {
    pub recovered: bool,
    pub missing_capabilities: Vec<String>,
    pub stale_workers: Vec<String>,
    pub failed_workers: Vec<String>,
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

/// Evaluate whether required runtime workers have recovered after crash/restart.
pub fn evaluate_worker_crash_recovery(
    input: WorkerCrashRecoveryEvaluationInput,
) -> WorkerCrashRecoveryEvaluation {
    let stale_after_seconds = input.stale_after_seconds.max(0);
    let cutoff = input.observed_at - chrono::Duration::seconds(stale_after_seconds);
    let mut missing_capabilities = Vec::new();
    let mut stale_workers = Vec::new();
    let mut failed_workers = Vec::new();

    for capability in &input.required_capabilities {
        let Some(observation) = input
            .observations
            .iter()
            .filter(|observation| &observation.capability == capability)
            .max_by_key(|observation| observation.last_heartbeat_at)
        else {
            missing_capabilities.push(capability.clone());
            continue;
        };
        if observation.status != HealthLevel::Healthy {
            failed_workers.push(observation.worker_id.clone());
            continue;
        }
        if observation
            .last_heartbeat_at
            .map(|last_heartbeat_at| last_heartbeat_at < cutoff)
            .unwrap_or(true)
        {
            stale_workers.push(observation.worker_id.clone());
        }
    }

    let recovered =
        missing_capabilities.is_empty() && stale_workers.is_empty() && failed_workers.is_empty();
    WorkerCrashRecoveryEvaluation {
        recovered,
        missing_capabilities,
        stale_workers,
        failed_workers,
        reason: if recovered {
            "all required workers have fresh healthy heartbeats".into()
        } else {
            "required worker missing, stale, or failed after crash recovery".into()
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
#[path = "runtime_tests.rs"]
mod runtime_tests;
