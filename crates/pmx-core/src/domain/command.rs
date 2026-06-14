use serde::{Deserialize, Serialize};

use crate::{AccountId, InternalOrderId, TradeIntent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionCommandKind {
    Place,
    Cancel,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionCommand {
    Place {
        intent: TradeIntent,
    },
    Cancel {
        account_id: AccountId,
        order_id: InternalOrderId,
    },
    Replace {
        account_id: AccountId,
        order_id: InternalOrderId,
        replacement: TradeIntent,
    },
}

impl ExecutionCommand {
    pub fn kind(&self) -> ExecutionCommandKind {
        match self {
            Self::Place { .. } => ExecutionCommandKind::Place,
            Self::Cancel { .. } => ExecutionCommandKind::Cancel,
            Self::Replace { .. } => ExecutionCommandKind::Replace,
        }
    }

    pub fn authorizes_remote_side_effect(&self) -> bool {
        false
    }
}
