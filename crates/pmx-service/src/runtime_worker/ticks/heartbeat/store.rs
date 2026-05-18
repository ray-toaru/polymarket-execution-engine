use super::*;

pub async fn record_heartbeat_lease_from_worker_status<S>(
    store: &S,
    tick: HeartbeatLeaseStoreTick,
) -> Result<HeartbeatLeaseStoreTickReceipt, ServiceError>
where
    S: RuntimeWorkerHealthStore
        + RuntimeWorkerObservationStore
        + RuntimeWorkerStatusStore
        + Send
        + Sync,
{
    if tick.account_id.trim().is_empty()
        || tick.provider_name.trim().is_empty()
        || tick.instance_id.trim().is_empty()
        || tick.status.trim().is_empty()
    {
        return Err(ServiceError::BadRequest(
            "account_id, provider_name, instance_id and status must be non-empty".into(),
        ));
    }
    if !tick.no_trading_side_effect {
        return Err(ServiceError::BadRequest(
            "heartbeat lease store ticks must not contain trading side effects".into(),
        ));
    }

    store
        .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
            worker_id: tick.instance_id.clone(),
            role: "HeartbeatLease".into(),
            capability: "heartbeat-lease".into(),
            status: tick.status.clone(),
            last_heartbeat_at: tick.observed_at,
            last_error: tick.last_error.clone(),
        })
        .await?;

    let status = store
        .list_runtime_worker_status(&RuntimeWorkerStatusQuery {
            account_id: tick.account_id.clone(),
            limit: 500,
            before_observed_at: None,
        })
        .await?;
    let candidates: Vec<HeartbeatLeaseCandidate> = status
        .heartbeats
        .into_iter()
        .filter(|heartbeat| heartbeat.capability == "heartbeat-lease")
        .map(|heartbeat| HeartbeatLeaseCandidate {
            worker_id: heartbeat.worker_id,
            status: heartbeat_health_level(&heartbeat.status),
            last_heartbeat_at: heartbeat.last_heartbeat_at,
            last_error: heartbeat.last_error,
        })
        .collect();
    let candidates_loaded = candidates.len();
    let receipt = record_heartbeat_lease_election_tick(
        store,
        HeartbeatLeaseElectionTick {
            account_id: tick.account_id,
            provider_name: tick.provider_name,
            instance_id: tick.instance_id.clone(),
            candidates,
            observed_at: tick.observed_at,
            stale_after_seconds: tick.stale_after_seconds,
            no_trading_side_effect: true,
        },
    )
    .await?;
    store
        .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
            worker_id: tick.instance_id,
            role: "HeartbeatLease".into(),
            capability: "heartbeat-lease".into(),
            status: tick.status,
            last_heartbeat_at: tick.observed_at,
            last_error: tick.last_error,
        })
        .await?;
    Ok(HeartbeatLeaseStoreTickReceipt {
        election: receipt.election,
        provider_tick: receipt.provider_tick,
        candidates_loaded,
        heartbeat_recorded: true,
    })
}
