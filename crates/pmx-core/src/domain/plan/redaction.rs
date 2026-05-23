use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{AccountId, InternalOrderId};

// Internal-only type. Do not expose in OpenAPI or public adapter clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedOrderEnvelope {
    pub internal_order_id: InternalOrderId,
    pub account_id: AccountId,
    pub signer_fingerprint: String,
    pub signed_payload_ref: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedactedPayloadEnvelope {
    pub schema_version: u32,
    pub kind: String,
    pub correlation_id: Option<String>,
    pub redacted_fields: Vec<String>,
    pub body: Value,
}

pub fn redacted_payload_envelope(
    kind: impl Into<String>,
    correlation_id: Option<String>,
    body: Value,
) -> Value {
    let envelope = RedactedPayloadEnvelope {
        schema_version: 1,
        kind: kind.into(),
        correlation_id,
        redacted_fields: vec![
            "private_key".into(),
            "clob_secret".into(),
            "signed_payload".into(),
            "signed_order_envelope".into(),
        ],
        body,
    };
    serde_json::to_value(envelope).expect("redacted payload envelope serializes")
}
