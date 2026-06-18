use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Scope {
    Service,
    Admin,
    AdminRead,
    AdminCancel,
    EmergencyOperator,
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
    #[error("admin read scope required")]
    AdminReadRequired,
    #[error("admin cancel scope required")]
    AdminCancelRequired,
    #[error("emergency operator scope required")]
    EmergencyOperatorRequired,
    #[error("service scope required")]
    ServiceRequired,
}

pub fn authorize(principal: &Principal, operation: Operation) -> Result<(), AuthzError> {
    let has_service = principal.scopes.contains(&Scope::Service);
    let has_admin = principal.scopes.contains(&Scope::Admin);
    let has_admin_read = has_admin || principal.scopes.contains(&Scope::AdminRead);
    let has_admin_cancel = has_admin || principal.scopes.contains(&Scope::AdminCancel);
    let has_emergency_operator = has_admin || principal.scopes.contains(&Scope::EmergencyOperator);
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
        Operation::ReadAudit => {
            if has_admin_read {
                Ok(())
            } else {
                Err(AuthzError::AdminReadRequired)
            }
        }
        Operation::CancelOrder | Operation::CancelMarket | Operation::Reconcile => {
            if has_admin_cancel {
                Ok(())
            } else {
                Err(AuthzError::AdminCancelRequired)
            }
        }
        Operation::KillSwitch => {
            if has_emergency_operator {
                Ok(())
            } else {
                Err(AuthzError::EmergencyOperatorRequired)
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
            Err(AuthzError::AdminCancelRequired)
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

    #[test]
    fn admin_read_scope_cannot_cancel_or_kill_switch() {
        let p = Principal {
            subject: "admin-read".into(),
            scopes: vec![Scope::AdminRead],
        };
        assert!(authorize(&p, Operation::ReadAudit).is_ok());
        assert_eq!(
            authorize(&p, Operation::CancelOrder),
            Err(AuthzError::AdminCancelRequired)
        );
        assert_eq!(
            authorize(&p, Operation::KillSwitch),
            Err(AuthzError::EmergencyOperatorRequired)
        );
    }

    #[test]
    fn admin_cancel_scope_cannot_read_audit_or_kill_switch() {
        let p = Principal {
            subject: "admin-cancel".into(),
            scopes: vec![Scope::AdminCancel],
        };
        assert!(authorize(&p, Operation::CancelOrder).is_ok());
        assert!(authorize(&p, Operation::CancelMarket).is_ok());
        assert!(authorize(&p, Operation::Reconcile).is_ok());
        assert_eq!(
            authorize(&p, Operation::ReadAudit),
            Err(AuthzError::AdminReadRequired)
        );
        assert_eq!(
            authorize(&p, Operation::KillSwitch),
            Err(AuthzError::EmergencyOperatorRequired)
        );
    }

    #[test]
    fn emergency_operator_scope_is_limited_to_kill_switch() {
        let p = Principal {
            subject: "emergency".into(),
            scopes: vec![Scope::EmergencyOperator],
        };
        assert!(authorize(&p, Operation::KillSwitch).is_ok());
        assert_eq!(
            authorize(&p, Operation::ReadAudit),
            Err(AuthzError::AdminReadRequired)
        );
        assert_eq!(
            authorize(&p, Operation::CancelOrder),
            Err(AuthzError::AdminCancelRequired)
        );
    }
}
