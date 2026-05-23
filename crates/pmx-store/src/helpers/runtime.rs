use chrono::{Duration, Utc};
use pmx_core::{RuntimeStateSummary, WorkerStatus};

use crate::{RuntimeWorkerHeartbeat, RuntimeWorkerObservation};

pub const DEFAULT_RUNTIME_OBSERVATION_TTL_SECONDS: i64 = 120;
pub const RUNTIME_OBSERVATION_TTL_SECONDS: i64 = DEFAULT_RUNTIME_OBSERVATION_TTL_SECONDS;

#[path = "runtime/apply.rs"]
mod apply;

#[path = "runtime/freshness.rs"]
mod freshness;

#[path = "runtime/status.rs"]
mod status;

#[path = "runtime/truth.rs"]
mod truth;

pub use apply::*;
pub use freshness::*;
pub(crate) use status::*;
