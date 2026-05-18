use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::HealthLevel;

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
