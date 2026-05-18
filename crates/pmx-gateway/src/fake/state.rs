use crate::{GatewayError, RemoteOrder};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FakeGatewayFailure {
    #[default]
    None,
    RemoteRejected(String),
    RemoteUnknown(String),
    AuthenticationFailed,
}

impl FakeGatewayFailure {
    pub(crate) fn apply(&self) -> Result<(), GatewayError> {
        match self {
            Self::None => Ok(()),
            Self::RemoteRejected(reason) => Err(GatewayError::RemoteRejected(reason.clone())),
            Self::RemoteUnknown(reason) => Err(GatewayError::RemoteUnknown(reason.clone())),
            Self::AuthenticationFailed => Err(GatewayError::AuthenticationFailed),
        }
    }
}

#[derive(Default)]
pub(crate) struct FakeGatewayInner {
    pub(crate) orders: HashMap<String, RemoteOrder>,
    pub(crate) post_failure: FakeGatewayFailure,
    pub(crate) cancel_failure: FakeGatewayFailure,
    pub(crate) read_failure: FakeGatewayFailure,
}

#[derive(Default, Clone)]
pub struct FakeGateway {
    pub(crate) inner: Arc<Mutex<FakeGatewayInner>>,
}

impl FakeGateway {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_post_failure(self, failure: FakeGatewayFailure) -> Self {
        self.inner
            .lock()
            .expect("fake gateway mutex poisoned")
            .post_failure = failure;
        self
    }

    pub fn with_cancel_failure(self, failure: FakeGatewayFailure) -> Self {
        self.inner
            .lock()
            .expect("fake gateway mutex poisoned")
            .cancel_failure = failure;
        self
    }

    pub fn with_read_failure(self, failure: FakeGatewayFailure) -> Self {
        self.inner
            .lock()
            .expect("fake gateway mutex poisoned")
            .read_failure = failure;
        self
    }

    pub fn insert_remote_order_for_test(&self, order: RemoteOrder) {
        self.inner
            .lock()
            .expect("fake gateway mutex poisoned")
            .orders
            .insert(order.remote_order_id.0.clone(), order);
    }
}
