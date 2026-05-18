use crate::{
    CLOB_V2_COLLATERAL_SYMBOL, CLOB_V2_HOST, CLOB_V2_SIGNING_PROTOCOL, OfficialSdkAdapterError,
    OfficialSdkPlanOrder, OfficialSdkStandardSignOnlyConstruction, OfficialSdkStandardSignOnlyPlan,
    OfficialSdkStandardSignOnlyProfile, SignOnlyDryRunReceipt,
    mapping::official_sdk_plan_to_builder_mapping, sign_only_lifecycle_records_from_receipt,
};
use pmx_core::HashValue;
use sha2::{Digest, Sha256};

pub fn validate_standard_sign_only_profile(
    profile: &OfficialSdkStandardSignOnlyProfile,
) -> Result<(), OfficialSdkAdapterError> {
    if profile.clob_host != CLOB_V2_HOST {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "standard sign-only adapter must use the CLOB V2 production host".into(),
        ));
    }
    if profile.collateral_symbol != CLOB_V2_COLLATERAL_SYMBOL {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "standard sign-only adapter must use pUSD collateral metadata".into(),
        ));
    }
    if profile.signing_protocol != CLOB_V2_SIGNING_PROTOCOL {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "standard sign-only adapter must use CLOB V2 signing".into(),
        ));
    }
    if profile.exposes_raw_signed_order || profile.may_post_order || profile.may_cancel_order {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "standard sign-only adapter must not expose raw signed orders, post orders, or cancel orders".into(),
        ));
    }
    Ok(())
}

pub fn standard_sign_only_plan_for_order(
    profile: OfficialSdkStandardSignOnlyProfile,
    plan: &OfficialSdkPlanOrder,
) -> Result<OfficialSdkStandardSignOnlyPlan, OfficialSdkAdapterError> {
    validate_standard_sign_only_profile(&profile)?;
    let mapping = official_sdk_plan_to_builder_mapping(plan)?;
    Ok(OfficialSdkStandardSignOnlyPlan {
        profile,
        mapping,
        signed_order_ref_namespace: "sign-only".into(),
        exposes_raw_signed_order: false,
        may_post_order: false,
        may_cancel_order: false,
    })
}

pub fn standard_sign_only_default_plan_for_order(
    plan: &OfficialSdkPlanOrder,
) -> Result<OfficialSdkStandardSignOnlyPlan, OfficialSdkAdapterError> {
    standard_sign_only_plan_for_order(OfficialSdkStandardSignOnlyProfile::default(), plan)
}

pub fn standard_sign_only_construction_for_order(
    plan: &OfficialSdkPlanOrder,
    plan_hash: HashValue,
) -> Result<OfficialSdkStandardSignOnlyConstruction, OfficialSdkAdapterError> {
    let standard_plan = standard_sign_only_default_plan_for_order(plan)?;
    let signed_order_digest = standard_sign_only_digest(&standard_plan, &plan_hash)?;
    let signed_order_ref = format!(
        "{}:{}:{}:digest-{}",
        standard_plan.signed_order_ref_namespace,
        plan.execution_id.0,
        plan_hash.0,
        &signed_order_digest[..16]
    );
    let receipt = SignOnlyDryRunReceipt {
        account_id: plan.account_id.clone(),
        execution_id: plan.execution_id.clone(),
        plan_hash: plan_hash.clone(),
        signed_order_ref: signed_order_ref.clone(),
        posted: false,
    };
    let lifecycle_records = sign_only_lifecycle_records_from_receipt(&receipt)?;
    Ok(OfficialSdkStandardSignOnlyConstruction {
        plan: standard_plan,
        plan_hash,
        signed_order_ref,
        signed_order_digest,
        no_remote_side_effect: true,
        raw_signed_order_exposed: false,
        lifecycle_records,
    })
}

fn standard_sign_only_digest(
    plan: &OfficialSdkStandardSignOnlyPlan,
    plan_hash: &HashValue,
) -> Result<String, OfficialSdkAdapterError> {
    let payload = serde_json::json!({
        "plan_hash": plan_hash,
        "profile": plan.profile,
        "mapping": plan.mapping,
        "namespace": plan.signed_order_ref_namespace,
    });
    let bytes = serde_json::to_vec(&payload)
        .map_err(|err| OfficialSdkAdapterError::InvalidInput(err.to_string()))?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}"))
}
