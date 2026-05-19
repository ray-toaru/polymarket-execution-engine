use super::*;

pub fn replayed_submit_outcome(response_json: &str) -> Result<SubmitOutcome, ServiceError> {
    let receipt: SubmitReceipt = serde_json::from_str(response_json).map_err(|err| {
        ServiceError::Internal(format!("stored submit receipt is invalid: {err}"))
    })?;
    Ok(SubmitOutcome::Replayed(receipt))
}
