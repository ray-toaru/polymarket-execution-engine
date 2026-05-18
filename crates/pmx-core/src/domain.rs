use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CoreError {
    #[error("exactly one quantity bound is required")]
    QuantityBoundCardinality,
    #[error("decimal string is invalid: {0}")]
    InvalidDecimal(String),
    #[error("quantity must be a positive canonical decimal: {0}")]
    InvalidQuantity(String),
    #[error("limit_price must be a canonical decimal in (0, 1]: {0}")]
    InvalidLimitPrice(String),
    #[error("unsupported quantity bound for side: {0}")]
    UnsupportedQuantityBound(String),
    #[error("canonical JSON serialization failed: {0}")]
    CanonicalJson(String),
    #[error("invalid state transition: {from:?} -> {event:?}")]
    InvalidTransition {
        from: OrderLifecycleState,
        event: OrderEventKind,
    },
    #[error("invalid sign-only transition: {from:?} -> {event:?}")]
    InvalidSignOnlyTransition {
        from: SignOnlyLifecycleState,
        event: SignOnlyLifecycleEventKind,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConditionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TokenId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HashValue(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InternalOrderId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RemoteOrderId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimeInForce {
    Gtc,
    Fok,
    Gtd,
    Fak,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecimalString(pub String);

impl DecimalString {
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_decimal_string(&self.0)
    }

    pub fn validate_positive(&self) -> Result<(), CoreError> {
        validate_positive_decimal_string(&self.0)
    }

    pub fn validate_limit_price(&self) -> Result<(), CoreError> {
        validate_limit_price_decimal_string(&self.0)
    }
}

pub fn validate_decimal_string(raw: &str) -> Result<(), CoreError> {
    if raw.is_empty() || raw.trim() != raw {
        return Err(CoreError::InvalidDecimal(raw.to_string()));
    }
    if raw.contains('e') || raw.contains('E') || raw.contains('+') || raw.contains('-') {
        return Err(CoreError::InvalidDecimal(raw.to_string()));
    }
    let parts: Vec<&str> = raw.split('.').collect();
    if parts.len() > 2 || parts[0].is_empty() {
        return Err(CoreError::InvalidDecimal(raw.to_string()));
    }
    if !parts[0].chars().all(|c| c.is_ascii_digit()) {
        return Err(CoreError::InvalidDecimal(raw.to_string()));
    }
    if parts[0].len() > 1 && parts[0].starts_with('0') {
        return Err(CoreError::InvalidDecimal(raw.to_string()));
    }
    if parts.len() == 2 && (parts[1].is_empty() || !parts[1].chars().all(|c| c.is_ascii_digit())) {
        return Err(CoreError::InvalidDecimal(raw.to_string()));
    }
    Ok(())
}

pub fn validate_positive_decimal_string(raw: &str) -> Result<(), CoreError> {
    validate_decimal_string(raw)?;
    if is_zero_decimal(raw) {
        return Err(CoreError::InvalidQuantity(raw.to_string()));
    }
    Ok(())
}

pub fn validate_limit_price_decimal_string(raw: &str) -> Result<(), CoreError> {
    validate_decimal_string(raw).map_err(|_| CoreError::InvalidLimitPrice(raw.to_string()))?;
    if is_zero_decimal(raw) || !decimal_leq_one(raw) {
        return Err(CoreError::InvalidLimitPrice(raw.to_string()));
    }
    Ok(())
}

fn is_zero_decimal(raw: &str) -> bool {
    raw.chars().filter(|c| *c != '.').all(|c| c == '0')
}

fn decimal_leq_one(raw: &str) -> bool {
    let mut parts = raw.split('.');
    let int = parts.next().unwrap_or("");
    let frac = parts.next().unwrap_or("");
    match int {
        "0" => true,
        "1" => frac.chars().all(|c| c == '0'),
        _ => false,
    }
}

fn sort_json_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut sorted = Map::new();
            for (key, value) in entries {
                sorted.insert(key, sort_json_value(value));
            }
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(sort_json_value).collect()),
        other => other,
    }
}

pub fn canonical_json_string<T: Serialize>(value: &T) -> Result<String, CoreError> {
    let json_value =
        serde_json::to_value(value).map_err(|err| CoreError::CanonicalJson(err.to_string()))?;
    serde_json::to_string(&sort_json_value(json_value))
        .map_err(|err| CoreError::CanonicalJson(err.to_string()))
}

pub fn canonical_json_sha256<T: Serialize>(value: &T) -> Result<HashValue, CoreError> {
    let canonical = canonical_json_string(value)?;
    let digest = Sha256::digest(canonical.as_bytes());
    Ok(HashValue(to_lower_hex(&digest)))
}

fn to_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketRef {
    pub condition_id: ConditionId,
    pub slug: Option<String>,
    pub is_sports: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuantityIntent {
    pub max_notional: Option<DecimalString>,
    pub max_shares: Option<DecimalString>,
}

impl QuantityIntent {
    pub fn canonicalize(&self, side: &Side) -> Result<QuantityBound, CoreError> {
        let provided = self.max_notional.is_some() as u8 + self.max_shares.is_some() as u8;
        if provided != 1 {
            return Err(CoreError::QuantityBoundCardinality);
        }
        if let Some(v) = &self.max_notional {
            v.validate_positive()?;
        }
        if let Some(v) = &self.max_shares {
            v.validate_positive()?;
        }
        match (side, &self.max_notional, &self.max_shares) {
            (Side::Buy, Some(v), None) => Ok(QuantityBound::WorstCaseQuoteNotional(v.clone())),
            (Side::Sell, None, Some(v)) => Ok(QuantityBound::WorstCaseBaseShares(v.clone())),
            (Side::Buy, None, Some(v)) => Ok(QuantityBound::Unsupported(format!(
                "BUY max_shares requires an explicit quote conversion rule: {}",
                v.0
            ))),
            (Side::Sell, Some(v), None) => Ok(QuantityBound::Unsupported(format!(
                "SELL max_notional requires an explicit base conversion rule: {}",
                v.0
            ))),
            _ => Err(CoreError::QuantityBoundCardinality),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "amount", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuantityBound {
    WorstCaseQuoteNotional(DecimalString),
    WorstCaseBaseShares(DecimalString),
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TradeIntent {
    pub client_intent_id: String,
    pub account_id: AccountId,
    pub market: MarketRef,
    pub token_id: TokenId,
    pub side: Side,
    pub quantity: QuantityIntent,
    pub limit_price: DecimalString,
    pub time_in_force: TimeInForce,
    pub collateral_profile_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedIntent {
    pub normalized_intent_id: String,
    pub intent_hash: HashValue,
    pub account_id: AccountId,
    pub market: MarketRef,
    pub token_id: TokenId,
    pub side: Side,
    pub quantity_bound: QuantityBound,
    pub limit_price: DecimalString,
    pub time_in_force: TimeInForce,
    pub collateral_profile_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GeoblockStatus {
    Allowed,
    Blocked,
    Unknown,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkerStatus {
    Healthy,
    Degraded,
    Stale,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CollateralProfileStatus {
    Resolved,
    DefaultResolved,
    ExplicitMissing,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeStateSummary {
    pub geoblock_status: GeoblockStatus,
    pub worker_status: WorkerStatus,
    pub collateral_profile_status: CollateralProfileStatus,
    pub kill_switch_enabled: bool,
    pub required_capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeasibilitySnapshot {
    pub snapshot_id: String,
    pub snapshot_hash: HashValue,
    pub normalized_intent_id: String,
    pub runtime_state: RuntimeStateSummary,
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DecisionStatus {
    Allow,
    Block,
    CloseOnly,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BlockReason {
    KillSwitchOn,
    GeoblockBlocked,
    GeoblockUnknown,
    GeoblockError,
    WorkerDegraded,
    WorkerStale,
    WorkerUnknown,
    CollateralProfileMissing,
    CollateralProfileUnknown,
    UnsupportedQuantityBound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstraintDecision {
    pub decision_id: String,
    pub decision_hash: HashValue,
    pub status: DecisionStatus,
    pub reasons: Vec<BlockReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalReceipt {
    pub approval_id: String,
    pub approved_by: String,
    pub approved_at: DateTime<Utc>,
    pub approval_hash: HashValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPlanSummary {
    pub execution_id: String,
    pub account_id: AccountId,
    pub normalized_intent_id: String,
    pub snapshot_id: String,
    pub decision_id: String,
    pub plan_hash: HashValue,
    pub status: PlanStatus,
    pub max_exposure: DecimalString,
    pub explanation: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlanStatus {
    Ready,
    Blocked,
}

// Internal-only type. Do not expose in OpenAPI or public control-plane clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedOrderEnvelope {
    pub internal_order_id: InternalOrderId,
    pub account_id: AccountId,
    pub signer_fingerprint: String,
    pub signed_payload_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignOnlyLifecycleState {
    Planned,
    ReservationPrepared,
    SigningRequested,
    SignedDryRun,
    Failed,
    Abandoned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignOnlyLifecycleEventKind {
    PrepareReservation,
    RequestSigning,
    SignedWithoutPost,
    SigningFailed,
    Abandon,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignOnlyLifecycleRecord {
    pub execution_id: ExecutionId,
    pub account_id: AccountId,
    pub state: SignOnlyLifecycleState,
    pub event: SignOnlyLifecycleEventKind,
    /// Client-supplied idempotency key for this lifecycle append.
    /// Stores must scope it to the execution and reject reuse with a different event payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_event_id: Option<String>,
    pub signed_order_ref: Option<String>,
    pub no_remote_side_effect: bool,
    /// Server-assigned metadata. Clients may omit it on append requests; stores populate it on reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_id: Option<i64>,
    /// Server-assigned metadata. Clients may omit it on append requests; stores populate it on reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

pub fn sign_only_lifecycle_records_equivalent(
    left: &SignOnlyLifecycleRecord,
    right: &SignOnlyLifecycleRecord,
) -> bool {
    left.execution_id == right.execution_id
        && left.account_id == right.account_id
        && left.state == right.state
        && left.event == right.event
        && left.client_event_id == right.client_event_id
        && left.signed_order_ref == right.signed_order_ref
        && left.no_remote_side_effect == right.no_remote_side_effect
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SubmitStatus {
    Accepted,
    Posted,
    PartialRemoteUnknown,
    RemoteUnknown,
    Rejected,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitReceipt {
    pub execution_id: String,
    pub receipt_id: String,
    pub status: SubmitStatus,
    pub executor_version: String,
    pub contract_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CancelState {
    Requested,
    RemoteAccepted,
    ConfirmedCanceled,
    NotCanceled,
    RemoteUnknown,
    ReconcileRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelReceipt {
    pub cancel_id: String,
    pub order_id: String,
    pub state: CancelState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReservationState {
    Pending,
    Active,
    Released,
    Consumed,
    Orphaned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderReservation {
    pub reservation_id: String,
    pub account_id: AccountId,
    pub execution_id: ExecutionId,
    pub internal_order_id: Option<InternalOrderId>,
    pub quantity_bound: QuantityBound,
    pub state: ReservationState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderLifecycleState {
    Planned,
    Signed,
    PostRequested,
    Posted,
    PartiallyFilled,
    Filled,
    CancelRequested,
    CancelRemoteAccepted,
    CancelConfirmed,
    RemoteUnknown,
    PartialRemoteUnknown,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderEventKind {
    Signed,
    PostRequested,
    RemotePosted,
    RemoteRejected,
    RemoteUnknown,
    PartialFill,
    FullFill,
    CancelRequested,
    CancelRemoteAccepted,
    CancelConfirmed,
    ReconcileOpen,
    ReconcileMissing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KillSwitchRequest {
    pub enabled: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KillSwitchReceipt {
    pub enabled: bool,
    pub changed_at: DateTime<Utc>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconcileRequest {
    pub account_id: AccountId,
    pub execution_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_observation: Option<RemoteOrderObservation>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconcileReport {
    pub reconcile_id: String,
    pub status: String,
    pub checked_orders: u64,
    pub findings: Vec<String>,
}

pub fn cancel_state_from_lifecycle(state: &OrderLifecycleState) -> CancelState {
    match state {
        OrderLifecycleState::CancelRequested => CancelState::Requested,
        OrderLifecycleState::CancelRemoteAccepted => CancelState::RemoteAccepted,
        OrderLifecycleState::CancelConfirmed => CancelState::ConfirmedCanceled,
        OrderLifecycleState::RemoteUnknown | OrderLifecycleState::PartialRemoteUnknown => {
            CancelState::RemoteUnknown
        }
        OrderLifecycleState::Failed => CancelState::NotCanceled,
        _ => CancelState::ReconcileRequired,
    }
}

pub fn lifecycle_requires_reconcile(state: &OrderLifecycleState) -> bool {
    matches!(
        state,
        OrderLifecycleState::RemoteUnknown | OrderLifecycleState::PartialRemoteUnknown
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReconcileAction {
    Noop,
    QueryRemoteOpenOrder,
    ConfirmMissingOrEscalate,
    OperatorRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RemoteOrderObservation {
    Open,
    Missing,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderLifecycleDivergenceKind {
    None,
    LocalRemoteUnknownRemoteOpen,
    LocalRemoteUnknownRemoteMissing,
    LocalRemoteUnknownStillUnknown,
    TerminalLocalRemoteMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderLifecycleDivergence {
    pub kind: OrderLifecycleDivergenceKind,
    pub event: Option<OrderEventKind>,
    pub operator_required: bool,
    pub no_remote_side_effect: bool,
    pub reason: String,
}

pub fn reconcile_action_for_lifecycle(state: &OrderLifecycleState) -> ReconcileAction {
    match state {
        OrderLifecycleState::RemoteUnknown => ReconcileAction::QueryRemoteOpenOrder,
        OrderLifecycleState::PartialRemoteUnknown => ReconcileAction::ConfirmMissingOrEscalate,
        OrderLifecycleState::Failed => ReconcileAction::OperatorRequired,
        _ => ReconcileAction::Noop,
    }
}

pub fn classify_order_lifecycle_divergence(
    local: &OrderLifecycleState,
    remote: RemoteOrderObservation,
) -> OrderLifecycleDivergence {
    match (local, remote) {
        (OrderLifecycleState::RemoteUnknown, RemoteOrderObservation::Open)
        | (OrderLifecycleState::PartialRemoteUnknown, RemoteOrderObservation::Open) => {
            OrderLifecycleDivergence {
                kind: OrderLifecycleDivergenceKind::LocalRemoteUnknownRemoteOpen,
                event: Some(OrderEventKind::ReconcileOpen),
                operator_required: false,
                no_remote_side_effect: true,
                reason: "remote order is open; restore local lifecycle to posted".into(),
            }
        }
        (OrderLifecycleState::RemoteUnknown, RemoteOrderObservation::Missing) => {
            OrderLifecycleDivergence {
                kind: OrderLifecycleDivergenceKind::LocalRemoteUnknownRemoteMissing,
                event: Some(OrderEventKind::ReconcileMissing),
                operator_required: false,
                no_remote_side_effect: true,
                reason: "first missing observation escalates to partial remote unknown".into(),
            }
        }
        (OrderLifecycleState::PartialRemoteUnknown, RemoteOrderObservation::Missing) => {
            OrderLifecycleDivergence {
                kind: OrderLifecycleDivergenceKind::LocalRemoteUnknownRemoteMissing,
                event: Some(OrderEventKind::ReconcileMissing),
                operator_required: true,
                no_remote_side_effect: true,
                reason: "second missing observation escalates to operator-required failed state"
                    .into(),
            }
        }
        (
            OrderLifecycleState::RemoteUnknown | OrderLifecycleState::PartialRemoteUnknown,
            RemoteOrderObservation::Unknown,
        ) => OrderLifecycleDivergence {
            kind: OrderLifecycleDivergenceKind::LocalRemoteUnknownStillUnknown,
            event: None,
            operator_required: true,
            no_remote_side_effect: true,
            reason: "remote truth remains unknown; operator review required".into(),
        },
        (
            OrderLifecycleState::Filled
            | OrderLifecycleState::CancelConfirmed
            | OrderLifecycleState::Failed,
            RemoteOrderObservation::Open,
        ) => OrderLifecycleDivergence {
            kind: OrderLifecycleDivergenceKind::TerminalLocalRemoteMismatch,
            event: None,
            operator_required: true,
            no_remote_side_effect: true,
            reason: "terminal local state conflicts with open remote observation".into(),
        },
        _ => OrderLifecycleDivergence {
            kind: OrderLifecycleDivergenceKind::None,
            event: None,
            operator_required: false,
            no_remote_side_effect: true,
            reason: "no lifecycle divergence requiring a local transition".into(),
        },
    }
}

pub fn transition_order_state(
    from: OrderLifecycleState,
    event: OrderEventKind,
) -> Result<OrderLifecycleState, CoreError> {
    let next = match (&from, &event) {
        (OrderLifecycleState::Planned, OrderEventKind::Signed) => OrderLifecycleState::Signed,
        (OrderLifecycleState::Signed, OrderEventKind::PostRequested) => {
            OrderLifecycleState::PostRequested
        }
        (OrderLifecycleState::PostRequested, OrderEventKind::RemotePosted) => {
            OrderLifecycleState::Posted
        }
        (OrderLifecycleState::PostRequested, OrderEventKind::RemoteRejected) => {
            OrderLifecycleState::Failed
        }
        (OrderLifecycleState::PostRequested, OrderEventKind::RemoteUnknown) => {
            OrderLifecycleState::RemoteUnknown
        }
        (OrderLifecycleState::Posted, OrderEventKind::PartialFill) => {
            OrderLifecycleState::PartiallyFilled
        }
        (OrderLifecycleState::Posted, OrderEventKind::FullFill) => OrderLifecycleState::Filled,
        (OrderLifecycleState::PartiallyFilled, OrderEventKind::PartialFill) => {
            OrderLifecycleState::PartiallyFilled
        }
        (OrderLifecycleState::PartiallyFilled, OrderEventKind::FullFill) => {
            OrderLifecycleState::Filled
        }
        (OrderLifecycleState::Posted, OrderEventKind::CancelRequested)
        | (OrderLifecycleState::PartiallyFilled, OrderEventKind::CancelRequested) => {
            OrderLifecycleState::CancelRequested
        }
        (OrderLifecycleState::CancelRequested, OrderEventKind::CancelRemoteAccepted) => {
            OrderLifecycleState::CancelRemoteAccepted
        }
        (OrderLifecycleState::CancelRequested, OrderEventKind::RemoteUnknown)
        | (OrderLifecycleState::CancelRemoteAccepted, OrderEventKind::RemoteUnknown) => {
            OrderLifecycleState::RemoteUnknown
        }
        (OrderLifecycleState::CancelRemoteAccepted, OrderEventKind::CancelConfirmed) => {
            OrderLifecycleState::CancelConfirmed
        }
        (OrderLifecycleState::RemoteUnknown, OrderEventKind::ReconcileOpen) => {
            OrderLifecycleState::Posted
        }
        (OrderLifecycleState::RemoteUnknown, OrderEventKind::ReconcileMissing) => {
            OrderLifecycleState::PartialRemoteUnknown
        }
        (OrderLifecycleState::PartialRemoteUnknown, OrderEventKind::ReconcileOpen) => {
            OrderLifecycleState::Posted
        }
        (OrderLifecycleState::PartialRemoteUnknown, OrderEventKind::ReconcileMissing) => {
            OrderLifecycleState::Failed
        }
        _ => return Err(CoreError::InvalidTransition { from, event }),
    };
    Ok(next)
}

pub fn transition_sign_only_lifecycle(
    from: SignOnlyLifecycleState,
    event: SignOnlyLifecycleEventKind,
) -> Result<SignOnlyLifecycleState, CoreError> {
    let next = match (&from, &event) {
        (SignOnlyLifecycleState::Planned, SignOnlyLifecycleEventKind::PrepareReservation) => {
            SignOnlyLifecycleState::ReservationPrepared
        }
        (
            SignOnlyLifecycleState::ReservationPrepared,
            SignOnlyLifecycleEventKind::RequestSigning,
        ) => SignOnlyLifecycleState::SigningRequested,
        (
            SignOnlyLifecycleState::SigningRequested,
            SignOnlyLifecycleEventKind::SignedWithoutPost,
        ) => SignOnlyLifecycleState::SignedDryRun,
        (SignOnlyLifecycleState::SigningRequested, SignOnlyLifecycleEventKind::SigningFailed)
        | (
            SignOnlyLifecycleState::ReservationPrepared,
            SignOnlyLifecycleEventKind::SigningFailed,
        ) => SignOnlyLifecycleState::Failed,
        (SignOnlyLifecycleState::Planned, SignOnlyLifecycleEventKind::Abandon)
        | (SignOnlyLifecycleState::ReservationPrepared, SignOnlyLifecycleEventKind::Abandon)
        | (SignOnlyLifecycleState::SigningRequested, SignOnlyLifecycleEventKind::Abandon) => {
            SignOnlyLifecycleState::Abandoned
        }
        _ => return Err(CoreError::InvalidSignOnlyTransition { from, event }),
    };
    Ok(next)
}

pub fn sign_only_lifecycle_has_remote_side_effect(record: &SignOnlyLifecycleRecord) -> bool {
    !record.no_remote_side_effect
}

pub fn normalize_intent(intent: TradeIntent) -> Result<NormalizedIntent, CoreError> {
    intent.limit_price.validate_limit_price()?;
    let quantity_bound = intent.quantity.canonicalize(&intent.side)?;
    let intent_hash = canonical_json_sha256(&intent)?;
    let normalized_intent_id = format!("norm-{}", intent_hash.0);
    Ok(NormalizedIntent {
        normalized_intent_id,
        intent_hash,
        account_id: intent.account_id,
        market: intent.market,
        token_id: intent.token_id,
        side: intent.side,
        quantity_bound,
        limit_price: intent.limit_price,
        time_in_force: intent.time_in_force,
        collateral_profile_id: intent.collateral_profile_id,
    })
}

#[cfg(test)]
#[path = "domain_tests.rs"]
mod domain_tests;
