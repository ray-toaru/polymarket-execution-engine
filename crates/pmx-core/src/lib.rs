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

pub fn reconcile_action_for_lifecycle(state: &OrderLifecycleState) -> ReconcileAction {
    match state {
        OrderLifecycleState::RemoteUnknown => ReconcileAction::QueryRemoteOpenOrder,
        OrderLifecycleState::PartialRemoteUnknown => ReconcileAction::ConfirmMissingOrEscalate,
        OrderLifecycleState::Failed => ReconcileAction::OperatorRequired,
        _ => ReconcileAction::Noop,
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
mod tests {
    use super::*;

    fn base_intent(side: Side, quantity: QuantityIntent) -> TradeIntent {
        TradeIntent {
            client_intent_id: "intent-1".into(),
            account_id: AccountId("acct-1".into()),
            market: MarketRef {
                condition_id: ConditionId("cond-1".into()),
                slug: None,
                is_sports: false,
            },
            token_id: TokenId("token-1".into()),
            side,
            quantity,
            limit_price: DecimalString("0.51".into()),
            time_in_force: TimeInForce::Gtc,
            collateral_profile_id: None,
        }
    }

    #[test]
    fn decimal_rejects_scientific_padding_and_trailing_dot() {
        for bad in ["", " 1", "1 ", "1e-3", "+1", "-1", ".5", "1.", "00.1"] {
            assert!(
                validate_decimal_string(bad).is_err(),
                "{bad} should be invalid"
            );
        }
        assert!(validate_decimal_string("0.5").is_ok());
    }

    #[test]
    fn limit_price_is_executor_authoritative() {
        for bad in ["0", "0.0", "1.01", "2", "1.0001"] {
            let mut intent = base_intent(
                Side::Buy,
                QuantityIntent {
                    max_notional: Some(DecimalString("10".into())),
                    max_shares: None,
                },
            );
            intent.limit_price = DecimalString(bad.into());
            assert!(matches!(
                normalize_intent(intent),
                Err(CoreError::InvalidLimitPrice(_))
            ));
        }
        let mut intent = base_intent(
            Side::Buy,
            QuantityIntent {
                max_notional: Some(DecimalString("10".into())),
                max_shares: None,
            },
        );
        intent.limit_price = DecimalString("1".into());
        assert!(normalize_intent(intent).is_ok());
    }

    #[test]
    fn quantity_must_be_positive() {
        let intent = base_intent(
            Side::Buy,
            QuantityIntent {
                max_notional: Some(DecimalString("0".into())),
                max_shares: None,
            },
        );
        assert!(matches!(
            normalize_intent(intent),
            Err(CoreError::InvalidQuantity(_))
        ));
    }

    #[test]
    fn quantity_requires_exactly_one_bound() {
        let intent = base_intent(
            Side::Buy,
            QuantityIntent {
                max_notional: None,
                max_shares: None,
            },
        );
        assert_eq!(
            normalize_intent(intent).unwrap_err(),
            CoreError::QuantityBoundCardinality
        );
    }

    #[test]
    fn buy_notional_canonicalizes_to_quote_bound() {
        let n = normalize_intent(base_intent(
            Side::Buy,
            QuantityIntent {
                max_notional: Some(DecimalString("10".into())),
                max_shares: None,
            },
        ))
        .unwrap();
        assert!(matches!(
            n.quantity_bound,
            QuantityBound::WorstCaseQuoteNotional(_)
        ));
    }

    #[test]
    fn sell_shares_canonicalizes_to_base_bound() {
        let n = normalize_intent(base_intent(
            Side::Sell,
            QuantityIntent {
                max_notional: None,
                max_shares: Some(DecimalString("7".into())),
            },
        ))
        .unwrap();
        assert!(matches!(
            n.quantity_bound,
            QuantityBound::WorstCaseBaseShares(_)
        ));
    }

    #[test]
    fn unsupported_cross_quantity_is_explicit() {
        let n = normalize_intent(base_intent(
            Side::Buy,
            QuantityIntent {
                max_notional: None,
                max_shares: Some(DecimalString("7".into())),
            },
        ))
        .unwrap();
        assert!(matches!(n.quantity_bound, QuantityBound::Unsupported(_)));
    }

    #[test]
    fn cannot_confirm_cancel_without_pending_cancel() {
        let err =
            transition_order_state(OrderLifecycleState::Posted, OrderEventKind::CancelConfirmed)
                .unwrap_err();
        assert!(matches!(err, CoreError::InvalidTransition { .. }));
    }

    #[test]
    fn cancel_confirmation_requires_remote_acceptance() {
        let s1 =
            transition_order_state(OrderLifecycleState::Posted, OrderEventKind::CancelRequested)
                .unwrap();
        let s2 = transition_order_state(s1, OrderEventKind::CancelRemoteAccepted).unwrap();
        let s3 = transition_order_state(s2, OrderEventKind::CancelConfirmed).unwrap();
        assert_eq!(s3, OrderLifecycleState::CancelConfirmed);
    }

    #[test]
    fn cancel_state_tracks_lifecycle_pending_and_terminal_states() {
        assert_eq!(
            cancel_state_from_lifecycle(&OrderLifecycleState::CancelRequested),
            CancelState::Requested
        );
        assert_eq!(
            cancel_state_from_lifecycle(&OrderLifecycleState::CancelRemoteAccepted),
            CancelState::RemoteAccepted
        );
        assert_eq!(
            cancel_state_from_lifecycle(&OrderLifecycleState::CancelConfirmed),
            CancelState::ConfirmedCanceled
        );
    }

    #[test]
    fn remote_unknown_states_require_reconcile() {
        let state = transition_order_state(
            OrderLifecycleState::PostRequested,
            OrderEventKind::RemoteUnknown,
        )
        .unwrap();
        assert!(lifecycle_requires_reconcile(&state));
        assert_eq!(
            cancel_state_from_lifecycle(&state),
            CancelState::RemoteUnknown
        );
    }

    #[test]
    fn canonical_json_hash_is_key_order_independent() {
        #[derive(Serialize)]
        struct Left {
            b: u8,
            a: u8,
        }
        #[derive(Serialize)]
        struct Right {
            a: u8,
            b: u8,
        }

        let left = canonical_json_sha256(&Left { b: 2, a: 1 }).unwrap();
        let right = canonical_json_sha256(&Right { a: 1, b: 2 }).unwrap();
        assert_eq!(left, right);
        assert_eq!(left.0.len(), 64);
    }

    #[test]
    fn normalized_intent_hash_is_content_derived() {
        let first = normalize_intent(base_intent(
            Side::Buy,
            QuantityIntent {
                max_notional: Some(DecimalString("10".into())),
                max_shares: None,
            },
        ))
        .unwrap();
        let second = normalize_intent(base_intent(
            Side::Buy,
            QuantityIntent {
                max_notional: Some(DecimalString("10".into())),
                max_shares: None,
            },
        ))
        .unwrap();
        assert_eq!(first.intent_hash, second.intent_hash);
        assert!(first.normalized_intent_id.starts_with("norm-"));
    }
    #[test]
    fn sign_only_lifecycle_never_models_remote_post() {
        let s1 = transition_sign_only_lifecycle(
            SignOnlyLifecycleState::Planned,
            SignOnlyLifecycleEventKind::PrepareReservation,
        )
        .unwrap();
        let s2 =
            transition_sign_only_lifecycle(s1, SignOnlyLifecycleEventKind::RequestSigning).unwrap();
        let s3 = transition_sign_only_lifecycle(s2, SignOnlyLifecycleEventKind::SignedWithoutPost)
            .unwrap();
        assert_eq!(s3, SignOnlyLifecycleState::SignedDryRun);
        let record = SignOnlyLifecycleRecord {
            execution_id: ExecutionId("exec-sign-only".into()),
            account_id: AccountId("acct-1".into()),
            state: s3,
            event: SignOnlyLifecycleEventKind::SignedWithoutPost,
            client_event_id: None,
            signed_order_ref: Some("sign-only:exec:hash:sig-abcd".into()),
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        };
        assert!(!sign_only_lifecycle_has_remote_side_effect(&record));
    }

    #[test]
    fn sign_only_lifecycle_rejects_direct_sign_without_reservation() {
        let err = transition_sign_only_lifecycle(
            SignOnlyLifecycleState::Planned,
            SignOnlyLifecycleEventKind::SignedWithoutPost,
        )
        .expect_err("direct sign-only completion must be invalid");
        assert!(matches!(err, CoreError::InvalidSignOnlyTransition { .. }));
    }

    #[test]
    fn reconcile_action_tracks_remote_unknown_and_partial_unknown() {
        assert_eq!(
            reconcile_action_for_lifecycle(&OrderLifecycleState::RemoteUnknown),
            ReconcileAction::QueryRemoteOpenOrder
        );
        assert_eq!(
            reconcile_action_for_lifecycle(&OrderLifecycleState::PartialRemoteUnknown),
            ReconcileAction::ConfirmMissingOrEscalate
        );
        assert_eq!(
            reconcile_action_for_lifecycle(&OrderLifecycleState::Posted),
            ReconcileAction::Noop
        );
    }
}
