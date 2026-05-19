use super::*;

pub fn request_fingerprint(req: &SubmitPlanCommand) -> Result<String, ServiceError> {
    Ok(canonical_json_sha256(req)
        .map_err(|err| ServiceError::Internal(err.to_string()))?
        .0)
}

pub fn response_fingerprint(receipt: &SubmitReceipt) -> Result<String, ServiceError> {
    Ok(canonical_json_sha256(receipt)
        .map_err(|err| ServiceError::Internal(err.to_string()))?
        .0)
}
