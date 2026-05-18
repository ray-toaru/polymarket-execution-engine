use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
pub enum WebSocketChannel {
    Market,
    User,
    Sports,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RuntimeWorkerKind {
    WebSocketLiveness,
    HeartbeatLease,
    Geoblock,
    ResourceRefresh,
    ReconcileBacklog,
}
