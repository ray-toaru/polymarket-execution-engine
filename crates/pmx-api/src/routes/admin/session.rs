use axum::{
    Json,
    http::{HeaderMap, StatusCode},
};
use pmx_authz::{Operation, authorize};

use crate::{model::AdminSessionResponse, support::ApiResult};

use super::super::require;

pub(crate) async fn get_admin_session(headers: HeaderMap) -> ApiResult<AdminSessionResponse> {
    let principal = require(&headers, Operation::ReadAudit)?;
    let capabilities = [
        Operation::ReadAudit,
        Operation::CancelOrder,
        Operation::CancelMarket,
        Operation::Reconcile,
        Operation::KillSwitch,
    ]
    .into_iter()
    .filter(|operation| authorize(&principal, operation.clone()).is_ok())
    .collect();
    Ok((
        StatusCode::OK,
        Json(AdminSessionResponse {
            principal_subject: principal.subject,
            scopes: principal.scopes,
            capabilities,
            no_remote_side_effect: true,
        }),
    ))
}
