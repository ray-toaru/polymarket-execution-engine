use pmx_core::{AccountId, ExecutionId, HashValue};
use pmx_official_sdk_adapter::{
    BuildRealFundsCanaryPreconditionsInput, LiveCanaryPreconditions, OfficialSdkAdapterConfig,
    RealFundsCanaryApproval, RealFundsCanaryMarketCandidate, RealFundsCanaryMarketDiagnostics,
    RealFundsCanaryReceipt, RealFundsCanaryRequest, RealFundsCanaryRiskLimits,
    RealFundsCanaryStageReport, ReviewedRealFundsCanaryReleaseDecision,
    build_real_funds_canary_preconditions,
    run_real_funds_canary_gtc_post_only_cancel_with_reporter,
    validate_real_funds_canary_market_with_diagnostics, validate_real_funds_canary_preconditions,
    validate_reviewed_real_funds_canary_release_decision,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs::OpenOptions, io::Write, path::PathBuf};

const ENV_ALLOW_REAL_FUNDS_CANARY: &str = "PMX_ALLOW_REAL_FUNDS_CANARY";

#[derive(Debug)]
struct Args {
    approval_file: PathBuf,
    artifact_sha256: String,
    evidence_manifest_sha256: String,
    idempotency_key: String,
    account_id: String,
    execution_id: String,
    plan_hash: String,
    daily_used_notional_usd: String,
    market_file: PathBuf,
    release_decision_file: Option<PathBuf>,
    runtime_truth_file: Option<PathBuf>,
    approval_consumed_marker: Option<PathBuf>,
    report_file: Option<PathBuf>,
    dry_run: bool,
    preflight_only: bool,
    armed: bool,
    allow_live_submit_config: bool,
    allow_real_funds_canary_config: bool,
}

#[derive(Debug, Serialize)]
struct CanaryCliReport {
    status: String,
    dry_run: bool,
    preflight_only: bool,
    armed: bool,
    selected_market_id_hash: Option<String>,
    selected_token_id_hash: Option<String>,
    limit_price: Option<String>,
    size: Option<String>,
    notional_usd: Option<String>,
    market_diagnostics: RealFundsCanaryMarketDiagnostics,
    approval_hash: String,
    artifact_bound: bool,
    evidence_manifest_bound: bool,
    market_candidate_sha256: String,
    market_candidate_bound: bool,
    release_decision_bound: bool,
    live_submit_allowed: bool,
    real_funds_canary_allowed: bool,
    posted: bool,
    remote_side_effects: bool,
    raw_signed_order_exposed: bool,
}

#[derive(Debug, Default)]
struct RuntimeTruthBindings {
    kill_switch: bool,
    live_submit_gate: bool,
    idempotency_lease: bool,
    order_cancel_reconciliation: bool,
}

#[derive(Debug, Deserialize)]
struct RuntimeTruthFile {
    schema_version: u64,
    dependencies: Vec<RuntimeTruthDependency>,
}

#[derive(Debug, Deserialize)]
struct RuntimeTruthDependency {
    name: String,
    status: String,
    evidence_ref: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = parse_args()?;
    if args.armed && args.dry_run {
        anyhow::bail!("--armed and --dry-run are mutually exclusive");
    }
    if args.armed && args.preflight_only {
        anyhow::bail!("--armed and --preflight-only are mutually exclusive");
    }
    let approval: RealFundsCanaryApproval =
        serde_json::from_str(&std::fs::read_to_string(&args.approval_file)?)?;
    let risk_limits = RealFundsCanaryRiskLimits {
        max_order_notional_usd: approval.max_order_notional_usd.clone(),
        max_daily_notional_usd: approval.max_daily_notional_usd.clone(),
        daily_used_notional_usd: args.daily_used_notional_usd.clone(),
    };
    let config = OfficialSdkAdapterConfig {
        allow_live_submit: args.allow_live_submit_config,
        allow_real_funds_canary: args.allow_real_funds_canary_config,
        ..OfficialSdkAdapterConfig::default()
    };
    let real_funds_env_enabled =
        std::env::var(ENV_ALLOW_REAL_FUNDS_CANARY).ok().as_deref() == Some("1");
    let market_candidate_bytes = std::fs::read(&args.market_file)?;
    let market_candidate_sha256 = sha256_hex(&market_candidate_bytes);
    let release_decision_bound = if args.armed || args.release_decision_file.is_some() {
        validate_reviewed_release_decision(&args, &approval, &market_candidate_sha256)?
    } else {
        false
    };
    let runtime_truth = load_runtime_truth_file(args.runtime_truth_file.as_ref())?;
    let market_candidate: RealFundsCanaryMarketCandidate =
        serde_json::from_slice(&market_candidate_bytes)?;
    let validation = validate_real_funds_canary_market_with_diagnostics(
        &config,
        &approval.max_order_notional_usd,
        market_candidate,
    )
    .await?;
    let Some(market) = validation.selection else {
        let report = CanaryCliReport {
            status: if args.armed {
                "armed_blocked_unsafe_market_candidate".into()
            } else {
                "dry_run_blocked_unsafe_market_candidate".into()
            },
            dry_run: args.dry_run,
            preflight_only: args.preflight_only,
            armed: args.armed,
            selected_market_id_hash: None,
            selected_token_id_hash: None,
            limit_price: None,
            size: None,
            notional_usd: None,
            market_diagnostics: validation.diagnostics,
            approval_hash: approval.approval_hash,
            artifact_bound: approval.artifact_sha256 == args.artifact_sha256,
            evidence_manifest_bound: approval.evidence_manifest_sha256
                == args.evidence_manifest_sha256,
            market_candidate_sha256: market_candidate_sha256.clone(),
            market_candidate_bound: approval.market_candidate_sha256 == market_candidate_sha256,
            release_decision_bound,
            live_submit_allowed: false,
            real_funds_canary_allowed: false,
            posted: false,
            remote_side_effects: false,
            raw_signed_order_exposed: false,
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    };
    let live_canary = LiveCanaryPreconditions {
        compile_feature_live_submit: cfg!(feature = "live-submit"),
        env_allow_live_submit: std::env::var("PMX_ALLOW_LIVE_SUBMIT").ok().as_deref() == Some("1"),
        config_allow_live_submit: args.allow_live_submit_config,
        kill_switch_open: std::env::var("PMX_KILL_SWITCH_OPEN").ok().as_deref() == Some("1"),
        runtime_worker_healthy: std::env::var("PMX_RUNTIME_WORKER_HEALTHY").ok().as_deref()
            == Some("1"),
        geoblock_allowed: std::env::var("PMX_GEOBLOCK_ALLOWED").ok().as_deref() == Some("1"),
        repository_reservation_exists: std::env::var("PMX_REPOSITORY_RESERVATION_EXISTS")
            .ok()
            .as_deref()
            == Some("1"),
        idempotency_key_written: std::env::var("PMX_IDEMPOTENCY_KEY_WRITTEN").ok().as_deref()
            == Some("1"),
        reconcile_worker_healthy: std::env::var("PMX_RECONCILE_WORKER_HEALTHY")
            .ok()
            .as_deref()
            == Some("1"),
        account_whitelisted: approval.account_id.0 == args.account_id,
        market_whitelisted: true,
        size_cap_ok: true,
        daily_cap_ok: true,
        operator_approved: !approval.operator_identity_ref.trim().is_empty(),
        cancel_only_fallback_ready: std::env::var("PMX_CANCEL_ONLY_FALLBACK_READY")
            .ok()
            .as_deref()
            == Some("1"),
    };
    let preconditions =
        build_real_funds_canary_preconditions(BuildRealFundsCanaryPreconditionsInput {
            approval: &approval,
            risk_limits: &risk_limits,
            market: &market,
            live_canary,
            artifact_sha256: &args.artifact_sha256,
            evidence_manifest_sha256: &args.evidence_manifest_sha256,
            market_candidate_sha256: &market_candidate_sha256,
            config_allow_real_funds_canary: args.allow_real_funds_canary_config,
            balance_allowance_checked: std::env::var("PMX_BALANCE_ALLOWANCE_CHECKED")
                .ok()
                .as_deref()
                == Some("1"),
            selected_market_safe: true,
            runtime_kill_switch_truth_bound: runtime_truth.kill_switch,
            runtime_live_submit_gate_bound: runtime_truth.live_submit_gate,
            runtime_idempotency_lease_bound: runtime_truth.idempotency_lease,
            runtime_order_cancel_reconciliation_bound: runtime_truth.order_cancel_reconciliation,
        });
    let request = RealFundsCanaryRequest {
        account_id: AccountId(args.account_id.clone()),
        execution_id: ExecutionId(args.execution_id.clone()),
        plan_hash: HashValue(args.plan_hash.clone()),
        idempotency_key: args.idempotency_key.clone(),
        approval: approval.clone(),
        risk_limits,
        market: market.clone(),
        market_candidate_sha256: market_candidate_sha256.clone(),
        preconditions,
    };

    if args.preflight_only {
        validate_real_funds_canary_preconditions(&config, &request)?;
        let report = CanaryCliReport {
            status: "preflight_ready".into(),
            dry_run: false,
            preflight_only: true,
            armed: false,
            selected_market_id_hash: Some(format!(
                "{:x}",
                sha2::Sha256::digest(market.market_id.as_bytes())
            )),
            selected_token_id_hash: Some(format!(
                "{:x}",
                sha2::Sha256::digest(market.token_id.as_bytes())
            )),
            limit_price: Some(market.limit_price),
            size: Some(market.size),
            notional_usd: Some(market.notional_usd),
            market_diagnostics: validation.diagnostics,
            approval_hash: approval.approval_hash,
            artifact_bound: approval.artifact_sha256 == args.artifact_sha256,
            evidence_manifest_bound: approval.evidence_manifest_sha256
                == args.evidence_manifest_sha256,
            market_candidate_sha256: market_candidate_sha256.clone(),
            market_candidate_bound: approval.market_candidate_sha256 == market_candidate_sha256,
            release_decision_bound,
            live_submit_allowed: true,
            real_funds_canary_allowed: real_funds_env_enabled
                && args.allow_real_funds_canary_config,
            posted: false,
            remote_side_effects: false,
            raw_signed_order_exposed: false,
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    if args.armed {
        validate_real_funds_canary_preconditions(&config, &request)?;
        create_approval_consumed_marker(&args, &approval, &market_candidate_sha256)?;
        let receipt =
            run_real_funds_canary_gtc_post_only_cancel_with_reporter(&config, request, |stage| {
                persist_stage_report(&args, stage)
            })
            .await?;
        persist_armed_report(&args, &receipt)?;
        println!("{}", serde_json::to_string_pretty(&receipt)?);
        return Ok(());
    }

    let report = CanaryCliReport {
        status: "dry_run_ready".into(),
        dry_run: true,
        preflight_only: false,
        armed: false,
        selected_market_id_hash: Some(format!(
            "{:x}",
            sha2::Sha256::digest(market.market_id.as_bytes())
        )),
        selected_token_id_hash: Some(format!(
            "{:x}",
            sha2::Sha256::digest(market.token_id.as_bytes())
        )),
        limit_price: Some(market.limit_price),
        size: Some(market.size),
        notional_usd: Some(market.notional_usd),
        market_diagnostics: validation.diagnostics,
        approval_hash: approval.approval_hash,
        artifact_bound: approval.artifact_sha256 == args.artifact_sha256,
        evidence_manifest_bound: approval.evidence_manifest_sha256 == args.evidence_manifest_sha256,
        market_candidate_sha256: market_candidate_sha256.clone(),
        market_candidate_bound: approval.market_candidate_sha256 == market_candidate_sha256,
        release_decision_bound,
        live_submit_allowed: false,
        real_funds_canary_allowed: real_funds_env_enabled && args.allow_real_funds_canary_config,
        posted: false,
        remote_side_effects: false,
        raw_signed_order_exposed: false,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn parse_args() -> anyhow::Result<Args> {
    let mut values = HashMap::<String, String>::new();
    let mut dry_run = true;
    let mut preflight_only = false;
    let mut armed = false;
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            "--armed" => {
                armed = true;
                dry_run = false;
            }
            "--preflight-only" => {
                preflight_only = true;
                dry_run = false;
            }
            "--allow-live-submit-config" => {
                values.insert(arg.to_string(), "true".into());
            }
            "--allow-real-funds-canary-config" => {
                values.insert(arg.to_string(), "true".into());
            }
            flag if flag.starts_with("--") => {
                let value = iter
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for {flag}"))?;
                values.insert(flag.into(), value);
            }
            _ => anyhow::bail!("unknown argument {arg}"),
        }
    }
    Ok(Args {
        approval_file: required(&values, "--approval-file")?.into(),
        artifact_sha256: required(&values, "--artifact-sha256")?,
        evidence_manifest_sha256: required(&values, "--evidence-manifest-sha256")?,
        idempotency_key: required(&values, "--idempotency-key")?,
        account_id: required(&values, "--account-id")?,
        execution_id: required(&values, "--execution-id")?,
        plan_hash: required(&values, "--plan-hash")?,
        market_file: required(&values, "--market-file")?.into(),
        release_decision_file: values.get("--release-decision-file").map(PathBuf::from),
        runtime_truth_file: values.get("--runtime-truth-file").map(PathBuf::from),
        approval_consumed_marker: values.get("--approval-consumed-marker").map(PathBuf::from),
        report_file: values.get("--report-file").map(PathBuf::from),
        daily_used_notional_usd: values
            .get("--daily-used-notional-usd")
            .cloned()
            .unwrap_or_else(|| "0".into()),
        dry_run,
        preflight_only,
        armed,
        allow_live_submit_config: values.contains_key("--allow-live-submit-config"),
        allow_real_funds_canary_config: values.contains_key("--allow-real-funds-canary-config"),
    })
}

fn required(values: &HashMap<String, String>, key: &str) -> anyhow::Result<String> {
    values
        .get(key)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("missing required argument {key}"))
}

fn validate_reviewed_release_decision(
    args: &Args,
    approval: &RealFundsCanaryApproval,
    market_candidate_sha256: &str,
) -> anyhow::Result<bool> {
    let path = args
        .release_decision_file
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("--release-decision-file is required with --armed"))?;
    let decision: ReviewedRealFundsCanaryReleaseDecision =
        serde_json::from_str(&std::fs::read_to_string(path)?)?;
    validate_reviewed_real_funds_canary_release_decision(
        &decision,
        approval,
        &args.artifact_sha256,
        &args.evidence_manifest_sha256,
        market_candidate_sha256,
    )?;
    Ok(true)
}

fn load_runtime_truth_file(path: Option<&PathBuf>) -> anyhow::Result<RuntimeTruthBindings> {
    let Some(path) = path else {
        return Ok(RuntimeTruthBindings::default());
    };
    let truth: RuntimeTruthFile = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    if truth.schema_version != 1 {
        anyhow::bail!(
            "unsupported runtime truth schema_version {}; expected 1",
            truth.schema_version
        );
    }

    let mut bindings = RuntimeTruthBindings::default();
    let mut invalid = Vec::<String>::new();
    for dependency in truth.dependencies {
        let bound = dependency.status == "durable_runtime_truth"
            && !dependency.evidence_ref.trim().is_empty()
            && !dependency.evidence_ref.contains("REPLACE_WITH");
        match dependency.name.as_str() {
            "kill_switch" => bindings.kill_switch = bound,
            "live_submit_gate" => bindings.live_submit_gate = bound,
            "idempotency_lease" => bindings.idempotency_lease = bound,
            "order_cancel_reconciliation" => bindings.order_cancel_reconciliation = bound,
            _ => continue,
        }
        if !bound {
            invalid.push(dependency.name);
        }
    }

    let missing = [
        ("kill_switch", bindings.kill_switch),
        ("live_submit_gate", bindings.live_submit_gate),
        ("idempotency_lease", bindings.idempotency_lease),
        (
            "order_cancel_reconciliation",
            bindings.order_cancel_reconciliation,
        ),
    ]
    .into_iter()
    .filter_map(|(name, bound)| if bound { None } else { Some(name) })
    .collect::<Vec<_>>();
    if !missing.is_empty() {
        anyhow::bail!(
            "runtime truth missing durable dependencies: {}; invalid bindings: {}",
            missing.join(","),
            invalid.join(",")
        );
    }
    Ok(bindings)
}

fn create_approval_consumed_marker(
    args: &Args,
    approval: &RealFundsCanaryApproval,
    market_candidate_sha256: &str,
) -> anyhow::Result<()> {
    let path = args.approval_consumed_marker.as_ref().ok_or_else(|| {
        anyhow::anyhow!("--approval-consumed-marker is required for armed real-funds canary")
    })?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let marker = json!({
        "approval_id": &approval.approval_id,
        "approval_hash": &approval.approval_hash,
        "market_candidate_sha256": market_candidate_sha256,
        "execution_id": &args.execution_id,
        "idempotency_key": &args.idempotency_key,
        "consumed_at": chrono::Utc::now().to_rfc3339(),
    });
    file.write_all(serde_json::to_string_pretty(&marker)?.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

fn persist_armed_report(args: &Args, receipt: &RealFundsCanaryReceipt) -> anyhow::Result<()> {
    write_report_file(args, receipt)
}

fn persist_stage_report(args: &Args, report: &RealFundsCanaryStageReport) -> anyhow::Result<()> {
    write_report_file(args, report)
}

fn write_report_file<T: Serialize>(args: &Args, report: &T) -> anyhow::Result<()> {
    let path = args
        .report_file
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("--report-file is required for armed real-funds canary"))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    file.write_all(serde_json::to_string_pretty(report)?.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_runtime_truth_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("pmx-{name}-{nonce}.json"))
    }

    #[test]
    fn runtime_truth_file_requires_all_durable_dependencies() {
        let path = temp_runtime_truth_path("partial-runtime-truth");
        std::fs::write(
            &path,
            r#"{"schema_version":1,"dependencies":[{"name":"kill_switch","status":"durable_runtime_truth","evidence_ref":"pg://kill"}]}"#,
        )
        .expect("write runtime truth");
        let err =
            load_runtime_truth_file(Some(&path)).expect_err("partial runtime truth must fail");
        let _ = std::fs::remove_file(&path);
        assert!(
            err.to_string()
                .contains("runtime truth missing durable dependencies")
        );
    }

    #[test]
    fn missing_runtime_truth_file_argument_fails_closed() {
        let truth = load_runtime_truth_file(None).expect("default runtime truth");
        assert!(!truth.kill_switch);
        assert!(!truth.live_submit_gate);
        assert!(!truth.idempotency_lease);
        assert!(!truth.order_cancel_reconciliation);
    }

    #[test]
    fn runtime_truth_file_sets_all_canary_precondition_booleans() {
        let path = temp_runtime_truth_path("complete-runtime-truth");
        std::fs::write(
            &path,
            r#"{
              "schema_version": 1,
              "dependencies": [
                {"name":"kill_switch","status":"durable_runtime_truth","evidence_ref":"pg://kill"},
                {"name":"live_submit_gate","status":"durable_runtime_truth","evidence_ref":"pg://live"},
                {"name":"idempotency_lease","status":"durable_runtime_truth","evidence_ref":"pg://idem"},
                {"name":"order_cancel_reconciliation","status":"durable_runtime_truth","evidence_ref":"pg://reconcile"}
              ]
            }"#,
        )
        .expect("write runtime truth");
        let truth = load_runtime_truth_file(Some(&path)).expect("runtime truth");
        let _ = std::fs::remove_file(&path);
        assert!(truth.kill_switch);
        assert!(truth.live_submit_gate);
        assert!(truth.idempotency_lease);
        assert!(truth.order_cancel_reconciliation);
    }
}
