use super::*;
use pmx_core::{AccountId, ExecutionId, GeoblockStatus, HashValue, SignOnlyLifecycleState};
use pmx_gateway::GatewayError;

#[cfg(feature = "sdk-typecheck")]
use polymarket_client_sdk_v2::error::Error as SdkError;

fn empty_credentials() -> AdapterCredentialSnapshot {
    AdapterCredentialSnapshot {
        has_l1_private_key: false,
        has_l2_api_key: false,
        has_l2_api_secret: false,
        has_l2_passphrase: false,
    }
}

fn l1_credentials() -> AdapterCredentialSnapshot {
    AdapterCredentialSnapshot {
        has_l1_private_key: true,
        has_l2_api_key: false,
        has_l2_api_secret: false,
        has_l2_passphrase: false,
    }
}

fn sample_plan_limit() -> OfficialSdkPlanOrder {
    OfficialSdkPlanOrder {
        execution_id: ExecutionId("exec-1".into()),
        account_id: AccountId("acct-1".into()),
        token_id: "123".into(),
        side: "buy".into(),
        order_kind: "limit".into(),
        limit_price: Some("0.55".into()),
        size: Some("10".into()),
        amount: None,
        time_in_force: Some("gtc".into()),
        expiration: None,
        post_only: Some(false),
        builder_attribution: None,
        fee_rate_bps: None,
        funder: None,
        signer: None,
        signature_type: None,
    }
}

#[path = "tests/canary.rs"]
mod canary;

#[path = "tests/real_funds.rs"]
mod real_funds;

#[path = "tests/feature_gated.rs"]
mod feature_gated;

#[path = "tests/liveness_errors.rs"]
mod liveness_errors;

#[path = "tests/mapping.rs"]
mod mapping;

fn all_live_canary_preconditions() -> LiveCanaryPreconditions {
    LiveCanaryPreconditions {
        compile_feature_live_submit: true,
        env_allow_live_submit: true,
        config_allow_live_submit: true,
        kill_switch_open: true,
        runtime_worker_healthy: true,
        geoblock_allowed: true,
        repository_reservation_exists: true,
        idempotency_key_written: true,
        reconcile_worker_healthy: true,
        account_whitelisted: true,
        market_whitelisted: true,
        size_cap_ok: true,
        daily_cap_ok: true,
        operator_approved: true,
        cancel_only_fallback_ready: true,
    }
}

#[path = "tests/sign_only.rs"]
mod sign_only;
