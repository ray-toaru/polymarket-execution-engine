use thiserror::Error;

use crate::{
    OrderEventKind, OrderLifecycleState, SignOnlyLifecycleEventKind, SignOnlyLifecycleState,
};

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
    #[error("hash value must be a lowercase 64-character sha256 hex string: {0}")]
    InvalidHashValue(String),
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
