use crate::{
    BuildRealFundsCanaryPreconditionsInput, LiveCanaryPreconditions, OfficialSdkAdapterConfig,
    RealFundsCanaryApproval, RealFundsCanaryMarketCandidate, RealFundsCanaryMarketDiagnostics,
    RealFundsCanaryReceipt, RealFundsCanaryRequest, RealFundsCanaryRiskLimits,
    RealFundsCanaryStageReport, ReviewedRealFundsCanaryReleaseDecision,
    build_real_funds_canary_preconditions, preflight_real_funds_canary_execution,
    run_real_funds_canary_gtc_post_only_cancel_with_reporter,
    validate_active_profile_env_for_canary, validate_real_funds_canary_market_with_diagnostics,
    validate_real_funds_canary_preconditions, validate_reviewed_real_funds_canary_release_decision,
};
use pmx_core::{AccountId, ExecutionId, HashValue};
use pmx_store::{CanaryRuntimeTruthQuery, CanaryRuntimeTruthStore, PostgresStore};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

const ENV_ALLOW_REAL_FUNDS_CANARY: &str = "PMX_ALLOW_REAL_FUNDS_CANARY";

#[derive(Debug)]
pub struct Args {
    approval_file: PathBuf,
    artifact_sha256: String,
    evidence_manifest_sha256: String,
    idempotency_key: String,
    account_id: String,
    execution_id: String,
    plan_hash: String,
    daily_used_notional_usd: String,
    env_file: Option<PathBuf>,
    market_file: PathBuf,
    release_decision_file: Option<PathBuf>,
    runtime_truth_file: Option<PathBuf>,
    runtime_truth_store: Option<String>,
    runtime_truth_database_url_env: Option<String>,
    runtime_truth_condition_id: Option<String>,
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
    runtime_kill_switch_truth_bound: bool,
    runtime_live_submit_gate_bound: bool,
    runtime_idempotency_lease_bound: bool,
    runtime_order_cancel_reconciliation_bound: bool,
    live_submit_allowed: bool,
    real_funds_canary_allowed: bool,
    preconditions_live_submit_would_pass: bool,
    preconditions_real_funds_canary_would_pass: bool,
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
    gate_snapshot: Option<RuntimeGateSnapshot>,
}

#[derive(Debug, Deserialize)]
struct RuntimeTruthFile {
    schema_version: u64,
    account_id: String,
    condition_id: String,
    artifact_sha256: String,
    workspace_manifest_sha256: String,
    archived_manifest_sha256: String,
    dependencies: Vec<RuntimeTruthDependency>,
    #[serde(default)]
    preflight_report: Option<RuntimeTruthPreflightReport>,
}

#[derive(Debug, Deserialize)]
struct RuntimeTruthDependency {
    name: String,
    status: String,
    evidence_ref: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RuntimeTruthPreflightReport {
    live_submit_allowed: bool,
    real_funds_canary_allowed: bool,
    preconditions_live_submit_would_pass: bool,
    preconditions_real_funds_canary_would_pass: bool,
    kill_switch_open: bool,
    runtime_worker_healthy: bool,
    geoblock_allowed: bool,
    repository_reservation_exists: bool,
    idempotency_key_written: bool,
    reconcile_worker_healthy: bool,
    cancel_only_fallback_ready: bool,
    balance_allowance_checked: bool,
    gate_evidence_refs: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
struct RuntimeGateSnapshot {
    kill_switch_open: Option<bool>,
    runtime_worker_healthy: Option<bool>,
    geoblock_allowed: Option<bool>,
    repository_reservation_exists: Option<bool>,
    idempotency_key_written: Option<bool>,
    reconcile_worker_healthy: Option<bool>,
    cancel_only_fallback_ready: Option<bool>,
    balance_allowance_checked: Option<bool>,
}

impl RuntimeTruthBindings {
    fn bool_or_false(&self, value: Option<bool>) -> bool {
        value.unwrap_or(false)
    }

    fn kill_switch_open(&self) -> bool {
        self.bool_or_false(
            self.gate_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.kill_switch_open),
        )
    }

    fn runtime_worker_healthy(&self) -> bool {
        self.bool_or_false(
            self.gate_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.runtime_worker_healthy),
        )
    }

    fn geoblock_allowed(&self) -> bool {
        self.bool_or_false(
            self.gate_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.geoblock_allowed),
        )
    }

    fn repository_reservation_exists(&self) -> bool {
        self.bool_or_false(
            self.gate_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.repository_reservation_exists),
        )
    }

    fn idempotency_key_written(&self) -> bool {
        self.bool_or_false(
            self.gate_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.idempotency_key_written),
        )
    }

    fn reconcile_worker_healthy(&self) -> bool {
        self.bool_or_false(
            self.gate_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.reconcile_worker_healthy),
        )
    }

    fn cancel_only_fallback_ready(&self) -> bool {
        self.bool_or_false(
            self.gate_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.cancel_only_fallback_ready),
        )
    }

    fn balance_allowance_checked(&self) -> bool {
        self.bool_or_false(
            self.gate_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.balance_allowance_checked),
        )
    }
}

pub async fn run(args: Args) -> anyhow::Result<()> {
    load_env_file(args.env_file.as_deref())?;
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
    validate_canary_input_bindings(&args, &approval, &market_candidate_sha256)?;
    let release_decision_bound = if args.armed || args.release_decision_file.is_some() {
        validate_reviewed_release_decision(&args, &approval, &market_candidate_sha256)?
    } else {
        false
    };
    let runtime_truth = load_runtime_truth(&args, &approval).await?;
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
            runtime_kill_switch_truth_bound: runtime_truth.kill_switch,
            runtime_live_submit_gate_bound: runtime_truth.live_submit_gate,
            runtime_idempotency_lease_bound: runtime_truth.idempotency_lease,
            runtime_order_cancel_reconciliation_bound: runtime_truth.order_cancel_reconciliation,
            live_submit_allowed: false,
            real_funds_canary_allowed: false,
            preconditions_live_submit_would_pass: false,
            preconditions_real_funds_canary_would_pass: false,
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
        kill_switch_open: runtime_truth.kill_switch_open(),
        runtime_worker_healthy: runtime_truth.runtime_worker_healthy(),
        geoblock_allowed: runtime_truth.geoblock_allowed(),
        repository_reservation_exists: runtime_truth.repository_reservation_exists(),
        idempotency_key_written: runtime_truth.idempotency_key_written(),
        reconcile_worker_healthy: runtime_truth.reconcile_worker_healthy(),
        account_whitelisted: approval.account_id.0 == args.account_id,
        market_whitelisted: true,
        size_cap_ok: true,
        daily_cap_ok: true,
        operator_approved: release_decision_bound
            && !approval.operator_identity_ref.trim().is_empty()
            && approval.operator_identity_sha256
                == format!("{:x}", Sha256::digest(approval.operator_identity_ref.as_bytes())),
        cancel_only_fallback_ready: runtime_truth.cancel_only_fallback_ready(),
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
            balance_allowance_checked: runtime_truth.balance_allowance_checked(),
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
        validate_active_profile_env_for_canary(&args.account_id)?;
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
            runtime_kill_switch_truth_bound: runtime_truth.kill_switch,
            runtime_live_submit_gate_bound: runtime_truth.live_submit_gate,
            runtime_idempotency_lease_bound: runtime_truth.idempotency_lease,
            runtime_order_cancel_reconciliation_bound: runtime_truth.order_cancel_reconciliation,
            live_submit_allowed: false,
            real_funds_canary_allowed: false,
            preconditions_live_submit_would_pass: true,
            preconditions_real_funds_canary_would_pass: real_funds_env_enabled
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
        validate_active_profile_env_for_canary(&args.account_id)?;
        persist_stage_report(
            &args,
            &RealFundsCanaryStageReport::stage(
                &request,
                "armed_precheck_started",
                None,
                None,
                false,
                false,
                false,
            ),
        )?;
        if let Err(err) = preflight_real_funds_canary_execution(&config).await {
            persist_stage_report(
                &args,
                &RealFundsCanaryStageReport::blocked(
                    &request,
                    "armed_precheck_failed",
                    err.to_string(),
                ),
            )?;
            return Err(err);
        }
        create_approval_consumed_marker(&args, &approval, &market_candidate_sha256)?;
        persist_stage_report(
            &args,
            &RealFundsCanaryStageReport::stage(
                &request,
                "approval_consumed",
                None,
                None,
                false,
                false,
                false,
            ),
        )?;
        let mut last_remote_side_effect_stage: Option<RealFundsCanaryStageReport> = None;
        let result =
            run_real_funds_canary_gtc_post_only_cancel_with_reporter(&config, request, |stage| {
                if stage.remote_side_effects {
                    last_remote_side_effect_stage = Some(stage.clone());
                }
                persist_stage_report(&args, stage)
            })
            .await;
        let receipt = match result {
            Ok(receipt) => receipt,
            Err(err) => {
                recover_last_remote_side_effect_stage(
                    &args,
                    last_remote_side_effect_stage.as_ref(),
                    &err.to_string(),
                )?;
                return Err(err);
            }
        };
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
        runtime_kill_switch_truth_bound: runtime_truth.kill_switch,
        runtime_live_submit_gate_bound: runtime_truth.live_submit_gate,
        runtime_idempotency_lease_bound: runtime_truth.idempotency_lease,
        runtime_order_cancel_reconciliation_bound: runtime_truth.order_cancel_reconciliation,
        live_submit_allowed: false,
        real_funds_canary_allowed: false,
        preconditions_live_submit_would_pass: false,
        preconditions_real_funds_canary_would_pass: false,
        posted: false,
        remote_side_effects: false,
        raw_signed_order_exposed: false,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

pub fn parse_args_from<I>(args: I) -> anyhow::Result<Args>
where
    I: IntoIterator<Item = String>,
{
    let mut values = HashMap::<String, String>::new();
    let mut dry_run = true;
    let mut preflight_only = false;
    let mut armed = false;
    let mut iter = args.into_iter();
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
        env_file: values.get("--env-file").map(PathBuf::from),
        release_decision_file: values.get("--release-decision-file").map(PathBuf::from),
        runtime_truth_file: values.get("--runtime-truth-file").map(PathBuf::from),
        runtime_truth_store: values.get("--runtime-truth-store").cloned(),
        runtime_truth_database_url_env: values.get("--runtime-truth-database-url-env").cloned(),
        runtime_truth_condition_id: values.get("--runtime-truth-condition-id").cloned(),
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

fn load_env_file(path: Option<&Path>) -> anyhow::Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    dotenvy::from_path_override(path)
        .map(|_| ())
        .map_err(|err| anyhow::anyhow!("failed to load env file {}: {err}", path.display()))
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

fn validate_canary_input_bindings(
    args: &Args,
    approval: &RealFundsCanaryApproval,
    market_candidate_sha256: &str,
) -> anyhow::Result<()> {
    if approval.account_id.0 != args.account_id {
        anyhow::bail!(
            "approval account_id {} does not match canary account_id {}",
            approval.account_id.0,
            args.account_id
        );
    }
    if approval.artifact_sha256 != args.artifact_sha256 {
        anyhow::bail!(
            "approval artifact_sha256 {} does not match canary artifact_sha256 {}",
            approval.artifact_sha256,
            args.artifact_sha256
        );
    }
    if approval.evidence_manifest_sha256 != args.evidence_manifest_sha256 {
        anyhow::bail!(
            "approval evidence_manifest_sha256 {} does not match canary evidence_manifest_sha256 {}",
            approval.evidence_manifest_sha256,
            args.evidence_manifest_sha256
        );
    }
    if approval.market_candidate_sha256 != market_candidate_sha256 {
        anyhow::bail!(
            "approval market_candidate_sha256 {} does not match candidate market sha256 {}",
            approval.market_candidate_sha256,
            market_candidate_sha256
        );
    }
    if approval.execution_style != "GTC_LIMIT_POST_ONLY_CANCEL" {
        anyhow::bail!(
            "approval execution_style {} does not match required real-funds canary execution style",
            approval.execution_style
        );
    }
    Ok(())
}

async fn load_runtime_truth(
    args: &Args,
    approval: &RealFundsCanaryApproval,
) -> anyhow::Result<RuntimeTruthBindings> {
    match args.runtime_truth_store.as_deref() {
        None => load_runtime_truth_file(args.runtime_truth_file.as_ref(), args, approval),
        Some("postgres") => {
            let condition_id = args.runtime_truth_condition_id.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "--runtime-truth-condition-id is required with --runtime-truth-store postgres"
                )
            })?;
            let database_url_env = args.runtime_truth_database_url_env.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "--runtime-truth-database-url-env is required with --runtime-truth-store postgres"
                )
            })?;
            let database_url = std::env::var(database_url_env).map_err(|_| {
                anyhow::anyhow!("runtime truth database URL env {database_url_env} is not set")
            })?;
            if database_url.trim().is_empty() {
                anyhow::bail!("runtime truth database URL env {database_url_env} is empty");
            }
            let store = PostgresStore::new(database_url);
            let bindings = store
                .load_canary_runtime_truth(&CanaryRuntimeTruthQuery {
                    account_id: args.account_id.clone(),
                    condition_id: condition_id.clone(),
                    collateral_profile_id: None,
                })
                .await?;
            Ok(runtime_truth_from_store_bindings(bindings))
        }
        Some(other) => anyhow::bail!("unsupported --runtime-truth-store {other}"),
    }
}

fn runtime_truth_from_store_bindings(
    bindings: pmx_store::CanaryRuntimeTruthBindings,
) -> RuntimeTruthBindings {
    RuntimeTruthBindings {
        kill_switch: bindings.kill_switch_open,
        live_submit_gate: bindings.live_submit_gate_ready,
        idempotency_lease: bindings.idempotency_lease_ready,
        order_cancel_reconciliation: bindings.order_cancel_reconciliation_ready,
        gate_snapshot: Some(RuntimeGateSnapshot {
            kill_switch_open: Some(bindings.kill_switch_open),
            runtime_worker_healthy: bindings.runtime_worker_healthy,
            geoblock_allowed: bindings.geoblock_allowed,
            repository_reservation_exists: bindings.repository_reservation_exists,
            idempotency_key_written: bindings.idempotency_key_written,
            reconcile_worker_healthy: bindings.reconcile_worker_healthy,
            cancel_only_fallback_ready: bindings.cancel_only_fallback_ready,
            balance_allowance_checked: bindings.balance_allowance_checked,
        }),
    }
}

fn load_runtime_truth_file(
    path: Option<&PathBuf>,
    args: &Args,
    approval: &RealFundsCanaryApproval,
) -> anyhow::Result<RuntimeTruthBindings> {
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
    if truth.account_id.trim().is_empty() {
        anyhow::bail!("runtime truth account_id is required");
    }
    if truth.account_id != args.account_id {
        anyhow::bail!(
            "runtime truth account_id {} does not match canary account_id {}",
            truth.account_id,
            args.account_id
        );
    }
    let expected_condition_id = args.runtime_truth_condition_id.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "--runtime-truth-condition-id is required with --runtime-truth-file"
        )
    })?;
    if truth.condition_id.trim().is_empty() {
        anyhow::bail!("runtime truth condition_id is required");
    }
    if truth.condition_id != *expected_condition_id {
        anyhow::bail!(
            "runtime truth condition_id {} does not match expected runtime-truth-condition-id {}",
            truth.condition_id,
            expected_condition_id
        );
    }
    if truth.condition_id != approval.condition_id {
        anyhow::bail!(
            "runtime truth condition_id {} does not match approval condition_id {}",
            truth.condition_id,
            approval.condition_id
        );
    }
    if truth.artifact_sha256 != args.artifact_sha256 {
        anyhow::bail!(
            "runtime truth artifact_sha256 {} does not match canary artifact_sha256 {}",
            truth.artifact_sha256,
            args.artifact_sha256
        );
    }
    let expected_workspace_manifest_sha256 =
        approval.workspace_manifest_sha256.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "approval workspace_manifest_sha256 is required with --runtime-truth-file"
            )
        })?;
    if truth.workspace_manifest_sha256 != *expected_workspace_manifest_sha256 {
        anyhow::bail!(
            "runtime truth workspace_manifest_sha256 {} does not match approval workspace_manifest_sha256 {}",
            truth.workspace_manifest_sha256,
            expected_workspace_manifest_sha256
        );
    }
    if truth.archived_manifest_sha256 != args.evidence_manifest_sha256 {
        anyhow::bail!(
            "runtime truth archived_manifest_sha256 {} does not match canary evidence_manifest_sha256 {}",
            truth.archived_manifest_sha256,
            args.evidence_manifest_sha256
        );
    }
    if approval
        .archived_manifest_sha256
        .as_ref()
        .is_some_and(|value| &truth.archived_manifest_sha256 != value)
    {
        anyhow::bail!(
            "runtime truth archived_manifest_sha256 {} does not match approval archived_manifest_sha256 {}",
            truth.archived_manifest_sha256,
            approval.archived_manifest_sha256.as_deref().unwrap_or("")
        );
    }
    let approval_gate_snapshot = approval
        .runtime_gate_snapshot
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("approval runtime_gate_snapshot must be an object"))?;
    let approval_gate_evidence_refs = approval
        .runtime_gate_evidence_refs
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("approval runtime_gate_evidence_refs must be an object"))?;

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
    bindings.gate_snapshot = truth.preflight_report.map(|report| {
        let approval_bool = |field: &str| -> anyhow::Result<bool> {
            approval_gate_snapshot
                .get(field)
                .and_then(serde_json::Value::as_bool)
                .ok_or_else(|| anyhow::anyhow!("approval runtime_gate_snapshot.{field} must be boolean"))
        };
        let required_bool_matches = [
            (
                "live_submit_allowed",
                report.live_submit_allowed,
                approval_bool("live_submit_allowed")?,
            ),
            (
                "real_funds_canary_allowed",
                report.real_funds_canary_allowed,
                approval_bool("real_funds_canary_allowed")?,
            ),
            (
                "preconditions_live_submit_would_pass",
                report.preconditions_live_submit_would_pass,
                approval_bool("preconditions_live_submit_would_pass")?,
            ),
            (
                "preconditions_real_funds_canary_would_pass",
                report.preconditions_real_funds_canary_would_pass,
                approval_bool("preconditions_real_funds_canary_would_pass")?,
            ),
            (
                "kill_switch_open",
                report.kill_switch_open,
                approval_bool("kill_switch_open")?,
            ),
            (
                "runtime_worker_healthy",
                report.runtime_worker_healthy,
                approval_bool("runtime_worker_healthy")?,
            ),
            (
                "geoblock_allowed",
                report.geoblock_allowed,
                approval_bool("geoblock_allowed")?,
            ),
            (
                "repository_reservation_exists",
                report.repository_reservation_exists,
                approval_bool("repository_reservation_exists")?,
            ),
            (
                "idempotency_key_written",
                report.idempotency_key_written,
                approval_bool("idempotency_key_written")?,
            ),
            (
                "reconcile_worker_healthy",
                report.reconcile_worker_healthy,
                approval_bool("reconcile_worker_healthy")?,
            ),
            (
                "cancel_only_fallback_ready",
                report.cancel_only_fallback_ready,
                approval_bool("cancel_only_fallback_ready")?,
            ),
            (
                "balance_allowance_checked",
                report.balance_allowance_checked,
                approval_bool("balance_allowance_checked")?,
            ),
        ];
        for (field, runtime_truth_value, approval_value) in required_bool_matches {
            if runtime_truth_value != approval_value {
                anyhow::bail!(
                    "runtime truth preflight_report.{field} does not match approval runtime_gate_snapshot.{field}"
                );
            }
        }
        for field in [
            "live_submit_allowed",
            "real_funds_canary_allowed",
            "preconditions_live_submit_would_pass",
            "preconditions_real_funds_canary_would_pass",
            "kill_switch_open",
            "runtime_worker_healthy",
            "geoblock_allowed",
            "repository_reservation_exists",
            "idempotency_key_written",
            "reconcile_worker_healthy",
            "cancel_only_fallback_ready",
            "balance_allowance_checked",
        ] {
            let approval_evidence_ref = approval_gate_evidence_refs.get(field).and_then(serde_json::Value::as_str).ok_or_else(|| {
                anyhow::anyhow!("approval runtime_gate_evidence_refs.{field} must be a non-empty string")
            })?;
            let evidence_ref = report
                .gate_evidence_refs
                .get(field)
                .ok_or_else(|| anyhow::anyhow!("runtime truth preflight_report.gate_evidence_refs.{field} is required"))?;
            if evidence_ref.trim().is_empty() || evidence_ref.contains("REPLACE_WITH") {
                anyhow::bail!(
                    "runtime truth preflight_report.gate_evidence_refs.{field} must be concrete"
                );
            }
            if approval_evidence_ref.trim().is_empty() {
                anyhow::bail!("approval runtime_gate_evidence_refs.{field} must be a non-empty string");
            }
            if approval_evidence_ref != evidence_ref {
                anyhow::bail!(
                    "runtime truth preflight_report.gate_evidence_refs.{field} does not match approval runtime_gate_evidence_refs.{field}"
                );
            }
        }
        Ok(RuntimeGateSnapshot {
        kill_switch_open: Some(report.kill_switch_open),
        runtime_worker_healthy: Some(report.runtime_worker_healthy),
        geoblock_allowed: Some(report.geoblock_allowed),
        repository_reservation_exists: Some(report.repository_reservation_exists),
        idempotency_key_written: Some(report.idempotency_key_written),
        reconcile_worker_healthy: Some(report.reconcile_worker_healthy),
        cancel_only_fallback_ready: Some(report.cancel_only_fallback_ready),
        balance_allowance_checked: Some(report.balance_allowance_checked),
        })
    }).transpose()?;
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
    write_report_file(args, report)?;
    append_stage_history(args, report)
}

fn recover_last_remote_side_effect_stage(
    args: &Args,
    stage: Option<&RealFundsCanaryStageReport>,
    run_error: &str,
) -> anyhow::Result<()> {
    let Some(stage) = stage else {
        return Ok(());
    };
    if !stage.remote_side_effects && !stage.operator_required {
        return Ok(());
    }
    persist_stage_report(args, stage).map_err(|persist_err| {
        anyhow::anyhow!(
            "real-funds canary failed after remote-side-effect stage {}; recovery report persistence also failed: {}; original error: {}",
            stage.stage,
            persist_err,
            run_error
        )
    })
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

fn append_stage_history(args: &Args, report: &RealFundsCanaryStageReport) -> anyhow::Result<()> {
    let path = args
        .report_file
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("--report-file is required for armed real-funds canary"))?;
    let history_path = stage_history_path(path);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(history_path)?;
    file.write_all(serde_json::to_string(report)?.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

fn stage_history_path(path: &Path) -> PathBuf {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| format!("{extension}.stages.jsonl"))
        .unwrap_or_else(|| "stages.jsonl".into());
    path.with_extension(extension)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmx_official_sdk_adapter::{
        RealFundsCanaryMarketRejectionCounts, RealFundsCanaryMarketSelection,
        RealFundsCanaryPreconditions,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_runtime_truth_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("pmx-{name}-{nonce}.json"))
    }

    fn temp_report_path(name: &str) -> PathBuf {
        temp_runtime_truth_path(name)
    }

    fn passing_preconditions() -> RealFundsCanaryPreconditions {
        RealFundsCanaryPreconditions {
            live_canary: LiveCanaryPreconditions {
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
            },
            env_allow_real_funds_canary: true,
            config_allow_real_funds_canary: true,
            approval_valid: true,
            approval_scope_matches: true,
            approval_not_expired: true,
            artifact_bound: true,
            evidence_manifest_bound: true,
            market_candidate_bound: true,
            max_order_notional_ok: true,
            max_daily_notional_ok: true,
            execution_style_gtc_post_only_cancel: true,
            balance_allowance_checked: true,
            selected_market_safe: true,
            runtime_kill_switch_truth_bound: true,
            runtime_live_submit_gate_bound: true,
            runtime_idempotency_lease_bound: true,
            runtime_order_cancel_reconciliation_bound: true,
        }
    }

    fn empty_market_diagnostics() -> RealFundsCanaryMarketDiagnostics {
        RealFundsCanaryMarketDiagnostics {
            market_validation_complete: true,
            candidates_seen: 1,
            safe_candidates: 1,
            max_ask_size: None,
            min_spread_bps: None,
            min_order_size_blocks: false,
            rejection_counts: RealFundsCanaryMarketRejectionCounts {
                inactive: 0,
                not_accepting_orders: 0,
                closed: 0,
                archived: 0,
                wrong_side: 0,
                wrong_order_type: 0,
                missing_book_snapshot_timestamp: 0,
                missing_human_review_ref: 0,
                missing_or_zero_target_size: 0,
                spread_too_wide: 0,
                missing_or_zero_best_ask: 0,
                insufficient_ask_size: 0,
                min_order_size_above_order_size: 0,
                exchange_rule_snapshot_invalid: 0,
                post_only_not_bound: 0,
                notional_binding_mismatch: 0,
                notional_over_cap: 0,
            },
        }
    }

    fn minimal_args(extra: &[&str]) -> Vec<String> {
        let mut args = vec![
            "--approval-file",
            "approval.json",
            "--artifact-sha256",
            "artifact",
            "--evidence-manifest-sha256",
            "manifest",
            "--idempotency-key",
            "idem",
            "--account-id",
            "acct",
            "--execution-id",
            "exec",
            "--plan-hash",
            "hash",
            "--market-file",
            "market.json",
        ];
        args.extend_from_slice(extra);
        args.into_iter().map(ToOwned::to_owned).collect()
    }

    fn approval_fixture() -> RealFundsCanaryApproval {
        RealFundsCanaryApproval {
            approval_id: "approval-canary".into(),
            approval_hash: "a".repeat(64),
            account_id: AccountId("acct-canary".into()),
            condition_id: "cond-1".into(),
            scope: "REAL_FUNDS_CANARY".into(),
            expires_at: "2099-01-01T00:00:00Z".into(),
            artifact_sha256: "b".repeat(64),
            evidence_manifest_sha256: "c".repeat(64),
            workspace_manifest_sha256: Some("e".repeat(64)),
            archived_manifest_sha256: Some("c".repeat(64)),
            market_candidate_sha256: "d".repeat(64),
            max_order_notional_usd: "1".into(),
            max_daily_notional_usd: "5".into(),
            execution_style: "GTC_LIMIT_POST_ONLY_CANCEL".into(),
            operator_identity_ref: "operator-local-approval".into(),
            operator_identity_sha256:
                "1cde65add0b43ed4a85f3f2d9006e1cb9cb9f23709e893cd95359421301c6648".into(),
            runtime_gate_snapshot: serde_json::json!({
                "live_submit_allowed": false,
                "real_funds_canary_allowed": false,
                "preconditions_live_submit_would_pass": true,
                "preconditions_real_funds_canary_would_pass": true,
                "kill_switch_open": true,
                "runtime_worker_healthy": true,
                "geoblock_allowed": true,
                "repository_reservation_exists": true,
                "idempotency_key_written": true,
                "reconcile_worker_healthy": true,
                "cancel_only_fallback_ready": true,
                "balance_allowance_checked": true
            }),
            runtime_gate_evidence_refs: serde_json::json!({
                "live_submit_allowed": "approval://runtime-gates/live-submit-authorized",
                "real_funds_canary_allowed": "approval://runtime-gates/real-funds-canary-authorized",
                "preconditions_live_submit_would_pass": "pg://truth/preflight/live-submit-preconditions",
                "preconditions_real_funds_canary_would_pass": "pg://truth/preflight/real-funds-preconditions",
                "kill_switch_open": "pg://truth/runtime_accounts/kill-switch",
                "runtime_worker_healthy": "pg://truth/worker_health/runtime-worker",
                "geoblock_allowed": "pg://truth/compliance/geoblock",
                "repository_reservation_exists": "pg://truth/repository/reservation",
                "idempotency_key_written": "pg://truth/worker_health/idempotency-lease",
                "reconcile_worker_healthy": "pg://truth/worker_health/reconcile-worker",
                "cancel_only_fallback_ready": "pg://truth/operations/cancel-only-fallback",
                "balance_allowance_checked": "pg://truth/balances/allowance-check"
            }),
        }
    }

    fn runtime_truth_args() -> Args {
        parse_args_from(vec![
            "--approval-file".into(),
            "approval.json".into(),
            "--artifact-sha256".into(),
            "b".repeat(64),
            "--evidence-manifest-sha256".into(),
            "c".repeat(64),
            "--idempotency-key".into(),
            "idem".into(),
            "--account-id".into(),
            "acct-canary".into(),
            "--execution-id".into(),
            "exec".into(),
            "--plan-hash".into(),
            "hash".into(),
            "--market-file".into(),
            "market.json".into(),
            "--runtime-truth-condition-id".into(),
            "cond-1".into(),
        ])
        .expect("parse args")
    }

    #[test]
    fn parses_optional_env_file_path() {
        let args =
            parse_args_from(minimal_args(&["--env-file", "/tmp/pmx.env"])).expect("parse args");
        assert_eq!(args.env_file.as_deref(), Some(Path::new("/tmp/pmx.env")));
    }

    #[test]
    fn load_env_file_reads_explicit_path() {
        let path = temp_runtime_truth_path("canary-env-file");
        std::fs::write(&path, "PMX_TEST_ENV_FILE_FLAG=from-env-file\n").expect("write env file");
        unsafe {
            std::env::remove_var("PMX_TEST_ENV_FILE_FLAG");
        }
        load_env_file(Some(&path)).expect("load env file");
        assert_eq!(
            std::env::var("PMX_TEST_ENV_FILE_FLAG").as_deref(),
            Ok("from-env-file")
        );
        unsafe {
            std::env::remove_var("PMX_TEST_ENV_FILE_FLAG");
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn parses_postgres_runtime_truth_source_without_database_url_value() {
        let args = parse_args_from(minimal_args(&[
            "--runtime-truth-store",
            "postgres",
            "--runtime-truth-database-url-env",
            "PMX_TEST_DATABASE_URL",
            "--runtime-truth-condition-id",
            "cond-1",
        ]))
        .expect("parse args");
        assert_eq!(args.runtime_truth_store.as_deref(), Some("postgres"));
        assert_eq!(
            args.runtime_truth_database_url_env.as_deref(),
            Some("PMX_TEST_DATABASE_URL")
        );
        assert_eq!(args.runtime_truth_condition_id.as_deref(), Some("cond-1"));
    }

    #[test]
    fn store_runtime_truth_bindings_map_to_canary_precondition_booleans() {
        let truth = runtime_truth_from_store_bindings(pmx_store::CanaryRuntimeTruthBindings {
            kill_switch_open: true,
            live_submit_gate_ready: true,
            idempotency_lease_ready: false,
            order_cancel_reconciliation_ready: true,
            runtime_worker_healthy: Some(true),
            geoblock_allowed: Some(true),
            repository_reservation_exists: Some(true),
            idempotency_key_written: Some(false),
            reconcile_worker_healthy: Some(true),
            cancel_only_fallback_ready: Some(true),
            balance_allowance_checked: Some(true),
            evidence_refs: vec!["runtime-state://kill-switch".into()],
        });
        assert!(truth.kill_switch);
        assert!(truth.live_submit_gate);
        assert!(!truth.idempotency_lease);
        assert!(truth.order_cancel_reconciliation);
        assert_eq!(
            truth.gate_snapshot.as_ref().and_then(|snapshot| snapshot.kill_switch_open),
            Some(true)
        );
        assert_eq!(
            truth.gate_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.runtime_worker_healthy),
            Some(true)
        );
        assert_eq!(
            truth.gate_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.geoblock_allowed),
            Some(true)
        );
        assert_eq!(
            truth.gate_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.repository_reservation_exists),
            Some(true)
        );
        assert_eq!(
            truth.gate_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.idempotency_key_written),
            Some(false)
        );
        assert_eq!(
            truth.gate_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.reconcile_worker_healthy),
            Some(true)
        );
        assert_eq!(
            truth.gate_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.cancel_only_fallback_ready),
            Some(true)
        );
        assert_eq!(
            truth.gate_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.balance_allowance_checked),
            Some(true)
        );
    }

    #[test]
    fn runtime_truth_file_preflight_snapshot_overrides_env_gate_values() {
        let path = temp_runtime_truth_path("canary-runtime-truth-gates");
        let truth = serde_json::json!({
            "schema_version": 1,
            "account_id": "acct-canary",
            "condition_id": "cond-1",
            "artifact_sha256": "b".repeat(64),
            "workspace_manifest_sha256": "e".repeat(64),
            "archived_manifest_sha256": "c".repeat(64),
            "dependencies": [
                {"name": "kill_switch", "status": "durable_runtime_truth", "evidence_ref": "pg://truth/kill-switch"},
                {"name": "live_submit_gate", "status": "durable_runtime_truth", "evidence_ref": "pg://truth/live-submit"},
                {"name": "idempotency_lease", "status": "durable_runtime_truth", "evidence_ref": "pg://truth/idempotency"},
                {"name": "order_cancel_reconciliation", "status": "durable_runtime_truth", "evidence_ref": "pg://truth/reconcile"},
            ],
            "preflight_report": {
                "live_submit_allowed": false,
                "real_funds_canary_allowed": false,
                "preconditions_live_submit_would_pass": true,
                "preconditions_real_funds_canary_would_pass": true,
                "kill_switch_open": true,
                "runtime_worker_healthy": true,
                "geoblock_allowed": true,
                "repository_reservation_exists": true,
                "idempotency_key_written": true,
                "reconcile_worker_healthy": true,
                "cancel_only_fallback_ready": true,
                "balance_allowance_checked": true,
                "gate_evidence_refs": {
                    "live_submit_allowed": "approval://runtime-gates/live-submit-authorized",
                    "real_funds_canary_allowed": "approval://runtime-gates/real-funds-canary-authorized",
                    "preconditions_live_submit_would_pass": "pg://truth/preflight/live-submit-preconditions",
                    "preconditions_real_funds_canary_would_pass": "pg://truth/preflight/real-funds-preconditions",
                    "kill_switch_open": "pg://truth/runtime_accounts/kill-switch",
                    "runtime_worker_healthy": "pg://truth/worker_health/runtime-worker",
                    "geoblock_allowed": "pg://truth/compliance/geoblock",
                    "repository_reservation_exists": "pg://truth/repository/reservation",
                    "idempotency_key_written": "pg://truth/worker_health/idempotency-lease",
                    "reconcile_worker_healthy": "pg://truth/worker_health/reconcile-worker",
                    "cancel_only_fallback_ready": "pg://truth/operations/cancel-only-fallback",
                    "balance_allowance_checked": "pg://truth/balances/allowance-check",
                },
            }
        });
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&truth).expect("serialize truth file"),
        )
        .expect("write truth file");
        unsafe {
            std::env::set_var("PMX_KILL_SWITCH_OPEN", "0");
            std::env::set_var("PMX_CANCEL_ONLY_FALLBACK_READY", "0");
            std::env::set_var("PMX_BALANCE_ALLOWANCE_CHECKED", "0");
        }
        let args = runtime_truth_args();
        let loaded =
            load_runtime_truth_file(Some(&path), &args, &approval_fixture()).expect("load runtime truth");
        assert!(loaded.kill_switch_open());
        assert!(loaded.cancel_only_fallback_ready());
        assert!(loaded.balance_allowance_checked());
        unsafe {
            std::env::remove_var("PMX_KILL_SWITCH_OPEN");
            std::env::remove_var("PMX_CANCEL_ONLY_FALLBACK_READY");
            std::env::remove_var("PMX_BALANCE_ALLOWANCE_CHECKED");
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn cli_preflight_report_serializes_runtime_truth_bindings() {
        let report = CanaryCliReport {
            status: "preflight_ready".into(),
            dry_run: false,
            preflight_only: true,
            armed: false,
            selected_market_id_hash: Some("market-hash".into()),
            selected_token_id_hash: Some("token-hash".into()),
            limit_price: Some("0.02".into()),
            size: Some("5".into()),
            notional_usd: Some("0.10".into()),
            market_diagnostics: empty_market_diagnostics(),
            approval_hash: "a".repeat(64),
            artifact_bound: true,
            evidence_manifest_bound: true,
            market_candidate_sha256: "b".repeat(64),
            market_candidate_bound: true,
            release_decision_bound: true,
            runtime_kill_switch_truth_bound: true,
            runtime_live_submit_gate_bound: true,
            runtime_idempotency_lease_bound: true,
            runtime_order_cancel_reconciliation_bound: true,
            live_submit_allowed: true,
            real_funds_canary_allowed: true,
            preconditions_live_submit_would_pass: true,
            preconditions_real_funds_canary_would_pass: true,
            posted: false,
            remote_side_effects: false,
            raw_signed_order_exposed: false,
        };

        let json = serde_json::to_value(&report).expect("serialize report");
        assert_eq!(json["runtime_kill_switch_truth_bound"], true);
        assert_eq!(json["runtime_live_submit_gate_bound"], true);
        assert_eq!(json["runtime_idempotency_lease_bound"], true);
        assert_eq!(json["runtime_order_cancel_reconciliation_bound"], true);
    }

    #[test]
    fn runtime_truth_file_requires_gate_evidence_refs() {
        let path = temp_runtime_truth_path("canary-runtime-truth-missing-gate-evidence");
        let truth = serde_json::json!({
            "schema_version": 1,
            "account_id": "acct-canary",
            "condition_id": "cond-1",
            "artifact_sha256": "b".repeat(64),
            "workspace_manifest_sha256": "e".repeat(64),
            "archived_manifest_sha256": "c".repeat(64),
            "dependencies": [
                {"name": "kill_switch", "status": "durable_runtime_truth", "evidence_ref": "pg://truth/kill-switch"},
                {"name": "live_submit_gate", "status": "durable_runtime_truth", "evidence_ref": "pg://truth/live-submit"},
                {"name": "idempotency_lease", "status": "durable_runtime_truth", "evidence_ref": "pg://truth/idempotency"},
                {"name": "order_cancel_reconciliation", "status": "durable_runtime_truth", "evidence_ref": "pg://truth/reconcile"}
            ],
            "preflight_report": {
                "live_submit_allowed": false,
                "real_funds_canary_allowed": false,
                "preconditions_live_submit_would_pass": true,
                "preconditions_real_funds_canary_would_pass": true,
                "kill_switch_open": true,
                "runtime_worker_healthy": true,
                "geoblock_allowed": true,
                "repository_reservation_exists": true,
                "idempotency_key_written": true,
                "reconcile_worker_healthy": true,
                "cancel_only_fallback_ready": true,
                "balance_allowance_checked": true
            }
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&truth).expect("serialize truth file"))
            .expect("write truth file");
        let args = runtime_truth_args();
        let error = load_runtime_truth_file(Some(&path), &args, &approval_fixture())
            .expect_err("missing gate evidence refs must fail");
        assert!(
            error
                .to_string()
                .contains("runtime truth preflight_report.gate_evidence_refs.kill_switch_open is required")
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn runtime_truth_file_requires_matching_top_level_bindings() {
        let path = temp_runtime_truth_path("mismatched-runtime-truth-bindings");
        let truth = serde_json::json!({
            "schema_version": 1,
            "account_id": "acct-other",
            "condition_id": "cond-other",
            "artifact_sha256": "d".repeat(64),
            "workspace_manifest_sha256": "e".repeat(64),
            "archived_manifest_sha256": "f".repeat(64),
            "dependencies": [
                {"name": "kill_switch", "status": "durable_runtime_truth", "evidence_ref": "pg://truth/kill-switch"},
                {"name": "live_submit_gate", "status": "durable_runtime_truth", "evidence_ref": "pg://truth/live-submit"},
                {"name": "idempotency_lease", "status": "durable_runtime_truth", "evidence_ref": "pg://truth/idempotency"},
                {"name": "order_cancel_reconciliation", "status": "durable_runtime_truth", "evidence_ref": "pg://truth/reconcile"},
            ]
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&truth).expect("serialize truth file"))
            .expect("write truth file");
        let args = runtime_truth_args();
        let error = load_runtime_truth_file(Some(&path), &args, &approval_fixture())
            .expect_err("mismatched runtime truth bindings must fail");
        assert!(error.to_string().contains("runtime truth account_id acct-other does not match"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn runtime_truth_file_requires_matching_approval_gate_snapshot_and_evidence_refs() {
        let path = temp_runtime_truth_path("mismatched-runtime-truth-gate-snapshot");
        let truth = serde_json::json!({
            "schema_version": 1,
            "account_id": "acct-canary",
            "condition_id": "cond-1",
            "artifact_sha256": "b".repeat(64),
            "workspace_manifest_sha256": "e".repeat(64),
            "archived_manifest_sha256": "c".repeat(64),
            "dependencies": [
                {"name": "kill_switch", "status": "durable_runtime_truth", "evidence_ref": "pg://truth/kill-switch"},
                {"name": "live_submit_gate", "status": "durable_runtime_truth", "evidence_ref": "pg://truth/live-submit"},
                {"name": "idempotency_lease", "status": "durable_runtime_truth", "evidence_ref": "pg://truth/idempotency"},
                {"name": "order_cancel_reconciliation", "status": "durable_runtime_truth", "evidence_ref": "pg://truth/reconcile"}
            ],
            "preflight_report": {
                "live_submit_allowed": true,
                "real_funds_canary_allowed": false,
                "preconditions_live_submit_would_pass": true,
                "preconditions_real_funds_canary_would_pass": true,
                "kill_switch_open": true,
                "runtime_worker_healthy": true,
                "geoblock_allowed": true,
                "repository_reservation_exists": true,
                "idempotency_key_written": true,
                "reconcile_worker_healthy": true,
                "cancel_only_fallback_ready": true,
                "balance_allowance_checked": true,
                "gate_evidence_refs": {
                    "live_submit_allowed": "approval://runtime-gates/live-submit-authorized",
                    "real_funds_canary_allowed": "approval://runtime-gates/real-funds-canary-authorized",
                    "preconditions_live_submit_would_pass": "pg://truth/preflight/live-submit-preconditions",
                    "preconditions_real_funds_canary_would_pass": "pg://truth/preflight/real-funds-preconditions",
                    "kill_switch_open": "pg://truth/runtime_accounts/kill-switch",
                    "runtime_worker_healthy": "pg://truth/worker_health/runtime-worker",
                    "geoblock_allowed": "pg://truth/compliance/geoblock",
                    "repository_reservation_exists": "pg://truth/repository/reservation",
                    "idempotency_key_written": "pg://truth/worker_health/idempotency-lease",
                    "reconcile_worker_healthy": "pg://truth/worker_health/reconcile-worker",
                    "cancel_only_fallback_ready": "pg://truth/operations/cancel-only-fallback",
                    "balance_allowance_checked": "pg://truth/balances/allowance-check"
                }
            }
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&truth).expect("serialize truth file"))
            .expect("write truth file");
        let args = runtime_truth_args();
        let error = load_runtime_truth_file(Some(&path), &args, &approval_fixture())
            .expect_err("mismatched runtime truth approval gate snapshot must fail");
        assert!(error
            .to_string()
            .contains("runtime truth preflight_report.live_submit_allowed does not match approval runtime_gate_snapshot.live_submit_allowed"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn canary_input_bindings_fail_closed_before_dry_run_ready() {
        let args = parse_args_from(vec![
            "--approval-file".into(),
            "approval.json".into(),
            "--artifact-sha256".into(),
            "b".repeat(64),
            "--evidence-manifest-sha256".into(),
            "c".repeat(64),
            "--idempotency-key".into(),
            "idem".into(),
            "--account-id".into(),
            "acct-other".into(),
            "--execution-id".into(),
            "exec".into(),
            "--plan-hash".into(),
            "hash".into(),
            "--market-file".into(),
            "market.json".into(),
        ])
        .expect("parse args");
        let err = validate_canary_input_bindings(&args, &approval_fixture(), &"d".repeat(64))
            .expect_err("approval account mismatch must fail");
        assert!(err
            .to_string()
            .contains("approval account_id acct-canary does not match canary account_id acct-other"));
    }

    #[test]
    fn stage_report_persistence_keeps_append_only_jsonl_history() {
        let report_path = temp_report_path("canary-stage-report");
        let args = parse_args_from(minimal_args(&[
            "--report-file",
            report_path.to_str().unwrap(),
        ]))
        .expect("parse args");
        let request = RealFundsCanaryRequest {
            account_id: AccountId("acct-canary".into()),
            execution_id: ExecutionId("exec-canary".into()),
            plan_hash: HashValue("plan-hash".into()),
            idempotency_key: "idem-canary".into(),
            approval: RealFundsCanaryApproval {
                approval_id: "approval-canary".into(),
                approval_hash: "a".repeat(64),
                account_id: AccountId("acct-canary".into()),
                condition_id: "cond-1".into(),
                scope: "REAL_FUNDS_CANARY".into(),
                expires_at: "2099-01-01T00:00:00Z".into(),
                artifact_sha256: "b".repeat(64),
                evidence_manifest_sha256: "c".repeat(64),
                workspace_manifest_sha256: Some("e".repeat(64)),
                archived_manifest_sha256: Some("c".repeat(64)),
                market_candidate_sha256: "d".repeat(64),
                max_order_notional_usd: "1".into(),
                max_daily_notional_usd: "5".into(),
                execution_style: "GTC_LIMIT_POST_ONLY_CANCEL".into(),
                operator_identity_ref: "operator-local-approval".into(),
                operator_identity_sha256:
                    "1cde65add0b43ed4a85f3f2d9006e1cb9cb9f23709e893cd95359421301c6648"
                        .into(),
                runtime_gate_snapshot: serde_json::json!({}),
                runtime_gate_evidence_refs: serde_json::json!({}),
            },
            risk_limits: RealFundsCanaryRiskLimits {
                max_order_notional_usd: "1".into(),
                max_daily_notional_usd: "5".into(),
                daily_used_notional_usd: "0".into(),
            },
            market: RealFundsCanaryMarketSelection {
                market_id: "market".into(),
                token_id: "123".into(),
                limit_price: "0.10".into(),
                size: "5".into(),
                notional_usd: "0.50".into(),
                selection_reason: "unit-test".into(),
            },
            market_candidate_sha256: "d".repeat(64),
            preconditions: RealFundsCanaryPreconditions {
                live_canary: LiveCanaryPreconditions {
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
                },
                env_allow_real_funds_canary: true,
                config_allow_real_funds_canary: true,
                approval_valid: true,
                approval_scope_matches: true,
                approval_not_expired: true,
                artifact_bound: true,
                evidence_manifest_bound: true,
                market_candidate_bound: true,
                max_order_notional_ok: true,
                max_daily_notional_ok: true,
                execution_style_gtc_post_only_cancel: true,
                balance_allowance_checked: true,
                selected_market_safe: true,
                runtime_kill_switch_truth_bound: true,
                runtime_live_submit_gate_bound: true,
                runtime_idempotency_lease_bound: true,
                runtime_order_cancel_reconciliation_bound: true,
            },
        };
        let first = RealFundsCanaryStageReport::stage(
            &request,
            "post_accepted",
            Some("remote-1".into()),
            Some("Live".into()),
            true,
            false,
            false,
        );
        let second = RealFundsCanaryStageReport::operator_required(
            &request,
            "cancel_unknown",
            Some("remote-1".into()),
            Some("Live".into()),
            "cancel_order timed out",
        );

        persist_stage_report(&args, &first).expect("persist first stage");
        persist_stage_report(&args, &second).expect("persist second stage");

        let latest = std::fs::read_to_string(&report_path).expect("latest report");
        assert!(latest.contains("\"stage\": \"cancel_unknown\""));
        let history_path = report_path.with_extension("json.stages.jsonl");
        let history = std::fs::read_to_string(&history_path).expect("stage history");
        let lines = history.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"stage\":\"post_accepted\""));
        assert!(lines[1].contains("\"stage\":\"cancel_unknown\""));
        let _ = std::fs::remove_file(&report_path);
        let _ = std::fs::remove_file(&history_path);
    }

    #[test]
    fn remote_side_effect_recovery_rewrites_latest_handoff_report() {
        let report_path = temp_report_path("canary-recovery-report");
        let args = parse_args_from(minimal_args(&[
            "--report-file",
            report_path.to_str().unwrap(),
        ]))
        .expect("parse args");
        let request = RealFundsCanaryRequest {
            account_id: AccountId("acct-canary".into()),
            execution_id: ExecutionId("exec-canary".into()),
            plan_hash: HashValue("plan-hash".into()),
            idempotency_key: "idem-canary".into(),
            approval: RealFundsCanaryApproval {
                approval_id: "approval-canary".into(),
                approval_hash: "a".repeat(64),
                account_id: AccountId("acct-canary".into()),
                condition_id: "cond-1".into(),
                scope: "REAL_FUNDS_CANARY".into(),
                expires_at: "2099-01-01T00:00:00Z".into(),
                artifact_sha256: "b".repeat(64),
                evidence_manifest_sha256: "c".repeat(64),
                workspace_manifest_sha256: Some("e".repeat(64)),
                archived_manifest_sha256: Some("c".repeat(64)),
                market_candidate_sha256: "d".repeat(64),
                max_order_notional_usd: "1".into(),
                max_daily_notional_usd: "5".into(),
                execution_style: "GTC_LIMIT_POST_ONLY_CANCEL".into(),
                operator_identity_ref: "operator-local-approval".into(),
                operator_identity_sha256:
                    "1cde65add0b43ed4a85f3f2d9006e1cb9cb9f23709e893cd95359421301c6648"
                        .into(),
                runtime_gate_snapshot: serde_json::json!({}),
                runtime_gate_evidence_refs: serde_json::json!({}),
            },
            risk_limits: RealFundsCanaryRiskLimits {
                max_order_notional_usd: "1".into(),
                max_daily_notional_usd: "5".into(),
                daily_used_notional_usd: "0".into(),
            },
            market: RealFundsCanaryMarketSelection {
                market_id: "market".into(),
                token_id: "123".into(),
                limit_price: "0.10".into(),
                size: "5".into(),
                notional_usd: "0.50".into(),
                selection_reason: "unit-test".into(),
            },
            market_candidate_sha256: "d".repeat(64),
            preconditions: passing_preconditions(),
        };
        let stage = RealFundsCanaryStageReport::operator_required(
            &request,
            "cancel_unknown",
            Some("remote-1".into()),
            Some("Live".into()),
            "cancel_order timed out",
        );

        recover_last_remote_side_effect_stage(&args, Some(&stage), "run failed")
            .expect("recovery handoff persisted");

        let latest = std::fs::read_to_string(&report_path).expect("latest report");
        assert!(latest.contains("\"stage\": \"cancel_unknown\""));
        assert!(latest.contains("\"operator_required\": true"));
        let history_path = report_path.with_extension("json.stages.jsonl");
        let history = std::fs::read_to_string(&history_path).expect("stage history");
        assert_eq!(history.lines().count(), 1);
        let _ = std::fs::remove_file(&report_path);
        let _ = std::fs::remove_file(&history_path);
    }

    #[tokio::test]
    async fn postgres_runtime_truth_source_requires_explicit_condition_id() {
        let args = parse_args_from(minimal_args(&[
            "--runtime-truth-store",
            "postgres",
            "--runtime-truth-database-url-env",
            "PMX_TEST_DATABASE_URL",
        ]))
        .expect("parse args");
        let err = load_runtime_truth(&args, &approval_fixture())
            .await
            .expect_err("missing condition id must fail before database access");
        assert!(
            err.to_string()
                .contains("--runtime-truth-condition-id is required")
        );
    }

    #[tokio::test]
    async fn unsupported_runtime_truth_source_fails_closed() {
        let args =
            parse_args_from(minimal_args(&["--runtime-truth-store", "file"])).expect("parse args");
        let err = load_runtime_truth(&args, &approval_fixture())
            .await
            .expect_err("unsupported source must fail");
        assert!(
            err.to_string()
                .contains("unsupported --runtime-truth-store file")
        );
    }

    #[test]
    fn runtime_truth_file_requires_all_durable_dependencies() {
        let path = temp_runtime_truth_path("partial-runtime-truth");
        std::fs::write(
            &path,
            r#"{"schema_version":1,"account_id":"acct-canary","condition_id":"cond-1","artifact_sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","workspace_manifest_sha256":"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee","archived_manifest_sha256":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","dependencies":[{"name":"kill_switch","status":"durable_runtime_truth","evidence_ref":"pg://kill"}]}"#,
        )
        .expect("write runtime truth");
        let args = runtime_truth_args();
        let err = load_runtime_truth_file(Some(&path), &args, &approval_fixture())
            .expect_err("partial runtime truth must fail");
        let _ = std::fs::remove_file(&path);
        assert!(
            err.to_string()
                .contains("runtime truth missing durable dependencies")
        );
    }

    #[test]
    fn missing_runtime_truth_file_argument_fails_closed() {
        let args = parse_args_from(minimal_args(&[])).expect("parse args");
        let truth = load_runtime_truth_file(None, &args, &approval_fixture()).expect("default runtime truth");
        assert!(!truth.kill_switch);
        assert!(!truth.live_submit_gate);
        assert!(!truth.idempotency_lease);
        assert!(!truth.order_cancel_reconciliation);
        unsafe {
            std::env::set_var("PMX_KILL_SWITCH_OPEN", "1");
            std::env::set_var("PMX_CANCEL_ONLY_FALLBACK_READY", "1");
            std::env::set_var("PMX_BALANCE_ALLOWANCE_CHECKED", "1");
        }
        assert!(!truth.kill_switch_open());
        assert!(!truth.cancel_only_fallback_ready());
        assert!(!truth.balance_allowance_checked());
        unsafe {
            std::env::remove_var("PMX_KILL_SWITCH_OPEN");
            std::env::remove_var("PMX_CANCEL_ONLY_FALLBACK_READY");
            std::env::remove_var("PMX_BALANCE_ALLOWANCE_CHECKED");
        }
    }

    #[test]
    fn runtime_truth_file_sets_all_canary_precondition_booleans() {
        let path = temp_runtime_truth_path("complete-runtime-truth");
        std::fs::write(
            &path,
            r#"{
              "schema_version": 1,
              "account_id": "acct-canary",
              "condition_id": "cond-1",
              "artifact_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
              "workspace_manifest_sha256": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
              "archived_manifest_sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
              "dependencies": [
                {"name":"kill_switch","status":"durable_runtime_truth","evidence_ref":"pg://kill"},
                {"name":"live_submit_gate","status":"durable_runtime_truth","evidence_ref":"pg://live"},
                {"name":"idempotency_lease","status":"durable_runtime_truth","evidence_ref":"pg://idem"},
                {"name":"order_cancel_reconciliation","status":"durable_runtime_truth","evidence_ref":"pg://reconcile"}
              ]
            }"#,
        )
        .expect("write runtime truth");
        let args = runtime_truth_args();
        let truth =
            load_runtime_truth_file(Some(&path), &args, &approval_fixture()).expect("runtime truth");
        let _ = std::fs::remove_file(&path);
        assert!(truth.kill_switch);
        assert!(truth.live_submit_gate);
        assert!(truth.idempotency_lease);
        assert!(truth.order_cancel_reconciliation);
    }
}
