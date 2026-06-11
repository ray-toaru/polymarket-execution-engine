use axum::{
    Json,
    http::{HeaderMap, StatusCode},
};
use pmx_authz::Operation;

use crate::{model::AdminSessionResponse, support::ApiResult};

use super::super::require;

pub(crate) async fn get_admin_session(headers: HeaderMap) -> ApiResult<AdminSessionResponse> {
    let principal = require(&headers, Operation::ReadAudit)?;
    Ok((
        StatusCode::OK,
        Json(AdminSessionResponse {
            principal_subject: principal.subject,
            scopes: principal.scopes,
            capabilities: vec![
                Operation::ReadAudit,
                Operation::CancelOrder,
                Operation::CancelMarket,
                Operation::Reconcile,
                Operation::KillSwitch,
            ],
            no_remote_side_effect: true,
        }),
    ))
}
