use super::*;

pub fn request_fingerprint(req: &SubmitPlanCommand) -> Result<String, ServiceError> {
    #[derive(serde::Serialize)]
    struct IdempotentSubmitRequest<'a> {
        execution_id: &'a str,
        plan_hash: &'a str,
        idempotency_key: &'a str,
        mode: &'a SubmitMode,
    }

    let idempotent_request = IdempotentSubmitRequest {
        execution_id: &req.execution_id,
        plan_hash: &req.plan_hash,
        idempotency_key: &req.idempotency_key,
        mode: &req.mode,
    };
    Ok(canonical_json_sha256(&idempotent_request)
        .map_err(|err| ServiceError::Internal(err.to_string()))?
        .0)
}

pub fn response_fingerprint(receipt: &SubmitReceipt) -> Result<String, ServiceError> {
    Ok(canonical_json_sha256(receipt)
        .map_err(|err| ServiceError::Internal(err.to_string()))?
        .0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(correlation_id: &str) -> SubmitPlanCommand {
        SubmitPlanCommand {
            execution_id: "exec-fingerprint".into(),
            plan_hash: "a".repeat(64),
            idempotency_key: "idem-fingerprint".into(),
            mode: SubmitMode::BlockedDryRun,
            correlation_id: Some(correlation_id.into()),
        }
    }

    #[test]
    fn request_fingerprint_ignores_correlation_metadata() {
        assert_eq!(
            request_fingerprint(&command("corr-first")).expect("first fingerprint"),
            request_fingerprint(&command("corr-replay")).expect("replay fingerprint"),
        );
    }
}
