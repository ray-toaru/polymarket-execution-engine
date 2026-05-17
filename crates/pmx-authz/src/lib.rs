use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Scope {
    Service,
    Admin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Principal {
    pub subject: String,
    pub scopes: Vec<Scope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Operation {
    NormalizeIntent,
    CaptureSnapshot,
    EvaluateDecision,
    CompilePlan,
    SubmitPlan,
    ReadReport,
    ReadAudit,
    RecordSignOnlyLifecycle,
    CancelOrder,
    CancelMarket,
    Reconcile,
    KillSwitch,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AuthzError {
    #[error("admin scope required")]
    AdminRequired,
    #[error("service scope required")]
    ServiceRequired,
}

pub fn authorize(principal: &Principal, operation: Operation) -> Result<(), AuthzError> {
    let has_service = principal.scopes.contains(&Scope::Service);
    let has_admin = principal.scopes.contains(&Scope::Admin);
    match operation {
        Operation::NormalizeIntent
        | Operation::CaptureSnapshot
        | Operation::EvaluateDecision
        | Operation::CompilePlan
        | Operation::SubmitPlan
        | Operation::ReadReport
        | Operation::RecordSignOnlyLifecycle => {
            if has_service || has_admin {
                Ok(())
            } else {
                Err(AuthzError::ServiceRequired)
            }
        }
        Operation::ReadAudit
        | Operation::CancelOrder
        | Operation::CancelMarket
        | Operation::Reconcile
        | Operation::KillSwitch => {
            if has_admin {
                Ok(())
            } else {
                Err(AuthzError::AdminRequired)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_cannot_cancel() {
        let p = Principal {
            subject: "svc".into(),
            scopes: vec![Scope::Service],
        };
        assert_eq!(
            authorize(&p, Operation::CancelOrder),
            Err(AuthzError::AdminRequired)
        );
    }

    #[test]
    fn admin_can_cancel() {
        let p = Principal {
            subject: "admin".into(),
            scopes: vec![Scope::Admin],
        };
        assert!(authorize(&p, Operation::CancelOrder).is_ok());
    }
}
