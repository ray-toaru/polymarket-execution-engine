use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{HealthLevel, WebSocketChannel};

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
