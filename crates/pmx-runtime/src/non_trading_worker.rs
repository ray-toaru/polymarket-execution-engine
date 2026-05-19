use std::future::Future;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::time::{Duration, interval};

use crate::{WorkerHeartbeat, WorkerRole};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NonTradingHeartbeatWorkerConfig {
    pub worker_id: String,
    pub capability: String,
    pub interval_seconds: u64,
}

impl NonTradingHeartbeatWorkerConfig {
    pub fn heartbeat(worker_id: impl Into<String>) -> Self {
        Self {
            worker_id: worker_id.into(),
            capability: "heartbeat".into(),
            interval_seconds: 30,
        }
    }
}

pub fn non_trading_worker_heartbeat(
    config: &NonTradingHeartbeatWorkerConfig,
    observed_at: DateTime<Utc>,
) -> WorkerHeartbeat {
    WorkerHeartbeat {
        worker_id: config.worker_id.clone(),
        role: WorkerRole::Heartbeat,
        capability: config.capability.clone(),
        observed_at,
        last_error: None,
    }
}

pub async fn emit_non_trading_heartbeat<S, Fut>(
    config: &NonTradingHeartbeatWorkerConfig,
    observed_at: DateTime<Utc>,
    sink: S,
) where
    S: FnOnce(WorkerHeartbeat) -> Fut,
    Fut: Future<Output = ()>,
{
    sink(non_trading_worker_heartbeat(config, observed_at)).await;
}

/// Run a non-trading heartbeat loop and hand each heartbeat to the caller.
///
/// This runtime crate deliberately owns no database or network side effects.
/// The injected sink can persist the heartbeat in the service/store layer while
/// this loop stays safe to use in pre-live and sign-only modes.
pub async fn run_non_trading_heartbeat_worker<S, Fut>(
    config: NonTradingHeartbeatWorkerConfig,
    mut sink: S,
) where
    S: FnMut(WorkerHeartbeat) -> Fut,
    Fut: Future<Output = ()>,
{
    let interval_seconds = config.interval_seconds.max(1);
    let mut ticker = interval(Duration::from_secs(interval_seconds));
    loop {
        ticker.tick().await;
        emit_non_trading_heartbeat(&config, Utc::now(), &mut sink).await;
    }
}

#[deprecated(note = "use run_non_trading_heartbeat_worker with a service/store sink")]
pub async fn run_placeholder_worker(worker_id: String) {
    let config = NonTradingHeartbeatWorkerConfig::heartbeat(worker_id);
    run_non_trading_heartbeat_worker(config, |_| async {}).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn non_trading_worker_heartbeat_builds_persistable_heartbeat() {
        let observed_at = Utc::now();
        let heartbeat = non_trading_worker_heartbeat(
            &NonTradingHeartbeatWorkerConfig {
                worker_id: "worker-runtime-non-trading".into(),
                capability: "heartbeat-lease".into(),
                interval_seconds: 0,
            },
            observed_at,
        );

        assert_eq!(heartbeat.worker_id, "worker-runtime-non-trading");
        assert!(matches!(heartbeat.role, WorkerRole::Heartbeat));
        assert_eq!(heartbeat.capability, "heartbeat-lease");
        assert_eq!(heartbeat.observed_at, observed_at);
        assert!(heartbeat.last_error.is_none());
    }

    #[tokio::test]
    async fn emit_non_trading_heartbeat_delivers_to_sink_without_trading_side_effects() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let sink_capture = Arc::clone(&captured);

        emit_non_trading_heartbeat(
            &NonTradingHeartbeatWorkerConfig::heartbeat("worker-runtime-sink"),
            Utc::now(),
            move |heartbeat| {
                let sink_capture = Arc::clone(&sink_capture);
                async move {
                    sink_capture.lock().expect("capture lock").push(heartbeat);
                }
            },
        )
        .await;

        let captured = captured.lock().expect("capture lock");
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].worker_id, "worker-runtime-sink");
        assert_eq!(captured[0].capability, "heartbeat");
        assert!(captured[0].last_error.is_none());
    }
}
