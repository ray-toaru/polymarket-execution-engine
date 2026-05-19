use crate::backend::AppState;
use crate::model::*;
use crate::support::{ApiResult, require, service_error};
use axum::{Json, extract::State, http::HeaderMap, http::StatusCode};
use pmx_authz::Operation;
use pmx_core::*;
use pmx_service::{
    StandardSignOnlyConstructionReceipt, StandardSignOnlyConstructionRequest, SubmitOutcome,
};

#[path = "flow/intent.rs"]
mod intent;

#[path = "flow/plan.rs"]
mod plan;

#[path = "flow/sign_only.rs"]
mod sign_only;

pub(crate) use intent::{capture_snapshot, decide, normalize};
pub(crate) use plan::{compile_plan, submit_plan};
pub(crate) use sign_only::{
    record_sign_only_lifecycle_event, record_standard_sign_only_construction,
};
