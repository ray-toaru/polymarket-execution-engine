#!/usr/bin/env python3
"""Tests for store-truth CLI preflight evidence wiring."""
from __future__ import annotations

import json
import hashlib
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import check_current_evidence_manifest
import run_real_funds_canary_store_truth_cli_preflight
import write_current_evidence_manifest


class StoreTruthCliEvidenceTests(unittest.TestCase):
    def test_manifest_writer_has_dedicated_store_truth_cli_section(self) -> None:
        self.assertEqual(
            write_current_evidence_manifest.SECTIONS.get(
                "real_funds_canary_store_truth_cli_validation"
            ),
            ["72-real-funds-canary-store-truth-cli-preflight.log"],
        )

    def test_manifest_guard_requires_store_truth_cli_pass_semantics(self) -> None:
        rule = check_current_evidence_manifest.JSON_LOG_RULES.get(
            "72-real-funds-canary-store-truth-cli-preflight.log"
        )
        self.assertEqual(
            rule,
            {
                "status": "pass",
                "preflight_ready": True,
                "posted": False,
                "remote_side_effects": False,
                "raw_signed_order_exposed": False,
                "runtime_truth_source": "postgres",
                "selected_market_id_hash_present": True,
                "selected_token_id_hash_present": True,
            },
        )

    def test_manifest_guard_does_not_apply_pass_semantics_to_skipped_section(self) -> None:
        manifest = json.loads(check_current_evidence_manifest.TEMPLATE.read_text())
        manifest["real_funds_canary_store_truth_cli_validation"] = {
            "status": "skipped",
            "logs": [
                {
                    "path": (
                        "polymarket-execution-engine/evidence/current/logs/"
                        "72-real-funds-canary-store-truth-cli-preflight.log"
                    )
                }
            ],
        }
        with tempfile.TemporaryDirectory() as tmp_name:
            path = Path(tmp_name) / "manifest.json"
            path.write_text(json.dumps(manifest))
            with patch.object(
                check_current_evidence_manifest,
                "validate_json_log_semantics",
                side_effect=AssertionError("skipped section must not require pass semantics"),
            ):
                self.assertEqual(check_current_evidence_manifest.validate(path), 0)

    def test_manifest_writer_records_store_truth_cli_command_not_skip_fallback(self) -> None:
        self.assertEqual(
            write_current_evidence_manifest.LOG_COMMANDS.get(
                "72-real-funds-canary-store-truth-cli-preflight.log"
            ),
            "python validation/run_real_funds_canary_store_truth_cli_preflight.py",
        )

    def test_store_truth_preflight_can_emit_validator_compatible_runtime_truth(self) -> None:
        doc = run_real_funds_canary_store_truth_cli_preflight.runtime_truth_document(
            "acct-1",
            "cond-1",
            {
                "status": "preflight_ready",
                "posted": False,
                "remote_side_effects": False,
                "raw_signed_order_exposed": False,
            },
            artifact_sha256="1" * 64,
            workspace_manifest_sha256="2" * 64,
            archived_manifest_sha256="3" * 64,
        )
        dependencies = {item["name"]: item for item in doc["dependencies"]}
        self.assertEqual(
            set(dependencies),
            {
                "kill_switch",
                "live_submit_gate",
                "idempotency_lease",
                "order_cancel_reconciliation",
            },
        )
        self.assertTrue(all(item["status"] == "durable_runtime_truth" for item in dependencies.values()))
        self.assertTrue(all(item["evidence_ref"].startswith("pg://canary-runtime-truth/") for item in dependencies.values()))
        self.assertTrue(doc["references_only_no_secret_values"])
        self.assertEqual(doc["artifact_sha256"], "1" * 64)
        self.assertEqual(doc["workspace_manifest_sha256"], "2" * 64)
        self.assertEqual(doc["archived_manifest_sha256"], "3" * 64)
        self.assertFalse(doc["live_submit_allowed"])
        self.assertFalse(doc["remote_side_effects"])
        for field in [
            "posted",
            "remote_side_effects",
            "raw_signed_order_exposed",
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
        ]:
            self.assertIsInstance(doc["preflight_report"][field], bool)
        self.assertEqual(
            doc["preflight_report"]["gate_evidence_refs"]["kill_switch_open"],
            "pg://canary-runtime-truth/account/acct-1/condition/cond-1/runtime_accounts/kill-switch",
        )

    def test_runtime_truth_hash_inputs_must_be_sha256(self) -> None:
        with self.assertRaisesRegex(SystemExit, "must be 64-hex"):
            run_real_funds_canary_store_truth_cli_preflight.require_sha256("not-a-sha", "--artifact-sha256")

    def test_store_truth_candidate_binds_estimated_notional(self) -> None:
        candidate = run_real_funds_canary_store_truth_cli_preflight.market_candidate()
        self.assertEqual(candidate["limit_price"], "0.02")
        self.assertEqual(candidate["target_size"], "5")
        self.assertEqual(candidate["estimated_order_notional_usd"], "0.1")

    def test_store_truth_condition_id_uses_candidate_market_id(self) -> None:
        self.assertEqual(
            run_real_funds_canary_store_truth_cli_preflight.condition_id_from_candidate(
                {"market_id": "0xabc"}
            ),
            "0xabc",
        )

    def test_store_truth_candidate_sanitizes_review_only_outcome_for_cli(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_name:
            candidate_path = Path(tmp_name) / "candidate.json"
            candidate_path.write_text(
                json.dumps(
                    {
                        "market_id": "0xabc",
                        "token_id": "tok",
                        "outcome": "Yes",
                        "limit_price": "0.001",
                        "exchange_rule_snapshot": {"min_tick_size": "0.001"},
                    }
                )
            )
            with patch.dict(
                "run_real_funds_canary_store_truth_cli_preflight.os.environ",
                {"PMX_STORE_TRUTH_CANDIDATE_MARKET_FILE": str(candidate_path)},
                clear=True,
            ):
                candidate = run_real_funds_canary_store_truth_cli_preflight.market_candidate()
        selfNotIn = self.assertNotIn
        selfNotIn("outcome", candidate)
        self.assertEqual(candidate["estimated_order_notional_usd"], "0.005")
        self.assertEqual(candidate["exchange_rule_snapshot"]["min_tick_size"], "0.001")

    def test_store_truth_approval_keeps_single_attempt_notional_caps(self) -> None:
        approval = run_real_funds_canary_store_truth_cli_preflight.approval(
            "acct-1",
            "cond-1",
            "1" * 64,
            artifact_sha256="2" * 64,
            workspace_manifest_sha256="3" * 64,
            archived_manifest_sha256="4" * 64,
        )
        self.assertEqual(approval["condition_id"], "cond-1")
        self.assertEqual(approval["operator_identity_ref"], "local-store-truth-cli-preflight")
        self.assertEqual(
            approval["operator_identity_sha256"],
            hashlib.sha256(b"local-store-truth-cli-preflight").hexdigest(),
        )
        self.assertTrue(approval["runtime_gate_snapshot"]["kill_switch_open"])
        self.assertTrue(approval["runtime_gate_snapshot"]["live_submit_allowed"])
        self.assertTrue(approval["runtime_gate_snapshot"]["real_funds_canary_allowed"])
        self.assertIn("kill_switch_open", approval["runtime_gate_evidence_refs"])
        self.assertEqual(approval["max_order_notional_usd"], "1")
        self.assertEqual(approval["max_daily_notional_usd"], "1")

    def test_store_truth_cli_injects_synthetic_active_profile_env(self) -> None:
        class Result:
            returncode = 0
            stdout = json.dumps(
                {
                    "status": "preflight_ready",
                    "posted": False,
                    "remote_side_effects": False,
                    "raw_signed_order_exposed": False,
                    "live_submit_allowed": False,
                    "real_funds_canary_allowed": False,
                    "preconditions_live_submit_would_pass": True,
                    "preconditions_real_funds_canary_would_pass": True,
                    "selected_market_id_hash": "1" * 64,
                    "selected_token_id_hash": "2" * 64,
                }
            )
            stderr = ""

        with tempfile.TemporaryDirectory() as tmp_name:
            with patch("run_real_funds_canary_store_truth_cli_preflight.subprocess.run", return_value=Result()) as run_mock:
                run_real_funds_canary_store_truth_cli_preflight.run_cli(
                    Path(tmp_name),
                    "acct-store-truth-test",
                    "cond-store-truth-test",
                    artifact_sha256="b" * 64,
                    workspace_manifest_sha256="e" * 64,
                    archived_manifest_sha256="c" * 64,
                )
        env = run_mock.call_args.kwargs["env"]
        argv = run_mock.call_args.args[0]
        self.assertIn("--preflight-only", argv)
        self.assertIn("--allow-live-submit-config", argv)
        self.assertIn("--allow-real-funds-canary-config", argv)
        self.assertEqual(env["PMX_ACTIVE_ACCOUNT_PROFILE"], "store_truth_cli_preflight")
        self.assertEqual(env["PMX_ACTIVE_ACCOUNT_ID"], "acct-store-truth-test")
        self.assertEqual(env["PMX_ACTIVE_PROFILE_REF"], "local-profile://store_truth_cli_preflight")

    def test_load_env_file_expands_local_references(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_name:
            env_file = Path(tmp_name) / ".env"
            env_file.write_text(
                "\n".join(
                    [
                        "PMX_ACCT_B_CLOB_FUNDER=0x00000000000000000000000000000000000000b0",
                        "PMX_CLOB_FUNDER=${PMX_ACCT_B_CLOB_FUNDER}",
                        "POLY_API_KEY=${PMX_MISSING_FALLBACK}",
                    ]
                )
                + "\n"
            )
            with patch.dict(
                "run_real_funds_canary_store_truth_cli_preflight.os.environ",
                {},
                clear=True,
            ):
                run_real_funds_canary_store_truth_cli_preflight.load_env_file(env_file)
                self.assertEqual(
                    run_real_funds_canary_store_truth_cli_preflight.os.environ["PMX_CLOB_FUNDER"],
                    "0x00000000000000000000000000000000000000b0",
                )
                self.assertEqual(
                    run_real_funds_canary_store_truth_cli_preflight.os.environ["POLY_API_KEY"],
                    "${PMX_MISSING_FALLBACK}",
                )

    def test_load_default_env_files_only_loads_explicit_companion(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_name:
            tmp = Path(tmp_name)
            runtime_env = tmp / ".env.runtime"
            runtime_secrets = tmp / ".env.runtime.secrets"
            fallback_env = tmp / ".env"
            runtime_env.write_text("PMX_ACTIVE_ACCOUNT_PROFILE=acct_b\n")
            runtime_secrets.write_text("POLY_API_SECRET=secret\n")
            fallback_env.write_text("PMX_DATABASE_URL=postgres://pmx@localhost/pmx\n")
            with patch.object(run_real_funds_canary_store_truth_cli_preflight, "ROOT", tmp):
                with patch.dict(
                    "run_real_funds_canary_store_truth_cli_preflight.os.environ",
                    {},
                    clear=True,
                ):
                    run_real_funds_canary_store_truth_cli_preflight.load_default_env_files()
                    self.assertNotIn(
                        "POLY_API_SECRET",
                        run_real_funds_canary_store_truth_cli_preflight.os.environ,
                    )
                    self.assertNotIn(
                        "PMX_ACTIVE_ACCOUNT_PROFILE",
                        run_real_funds_canary_store_truth_cli_preflight.os.environ,
                    )
                    self.assertNotIn(
                        "PMX_DATABASE_URL",
                        run_real_funds_canary_store_truth_cli_preflight.os.environ,
                    )
                    run_real_funds_canary_store_truth_cli_preflight.load_default_env_files(
                        runtime_secrets_env_file=runtime_secrets
                    )
                    self.assertEqual(
                        run_real_funds_canary_store_truth_cli_preflight.os.environ[
                            "POLY_API_SECRET"
                        ],
                        "secret",
                    )

    def test_database_target_summary_redacts_password_but_keeps_endpoint(self) -> None:
        summary = run_real_funds_canary_store_truth_cli_preflight.database_target_summary(
            "postgres://pmx:secret@127.0.0.1:5433/pmx"
        )
        self.assertEqual(
            summary,
            {
                "scheme": "postgres",
                "hostname": "127.0.0.1",
                "port": 5433,
                "database": "pmx",
                "username": "pmx",
            },
        )

    @patch("run_real_funds_canary_store_truth_cli_preflight.subprocess.run")
    def test_database_connectivity_preflight_reports_target_and_empty_stderr(self, run_mock) -> None:
        run_mock.return_value.returncode = 2
        run_mock.return_value.stdout = ""
        run_mock.return_value.stderr = ""
        with self.assertRaises(SystemExit) as ctx:
            run_real_funds_canary_store_truth_cli_preflight.check_database_connectivity(
                "postgres://pmx:secret@127.0.0.1:5433/pmx"
            )
        payload = json.loads(str(ctx.exception))
        self.assertEqual(payload["stage"], "database_connectivity_preflight")
        self.assertEqual(payload["database_target"]["hostname"], "127.0.0.1")
        self.assertEqual(payload["database_target"]["port"], 5433)
        self.assertEqual(payload["database_target"]["database"], "pmx")
        self.assertEqual(payload["stderr"], "<empty>")

    @patch("run_real_funds_canary_store_truth_cli_preflight.subprocess.run")
    def test_seed_runtime_truth_failure_reports_target_and_returncode(self, run_mock) -> None:
        run_mock.return_value.returncode = 3
        run_mock.return_value.stdout = ""
        run_mock.return_value.stderr = "connection refused"
        with self.assertRaises(SystemExit) as ctx:
            run_real_funds_canary_store_truth_cli_preflight.run_psql(
                "postgres://pmx:secret@127.0.0.1:5433/pmx",
                "select 1;",
            )
        payload = json.loads(str(ctx.exception))
        self.assertEqual(payload["stage"], "seed_postgres_runtime_truth")
        self.assertEqual(payload["returncode"], 3)
        self.assertEqual(payload["database_target"]["hostname"], "127.0.0.1")
        self.assertEqual(payload["stderr"], "connection refused")

    def test_seed_runtime_truth_scopes_worker_rows_to_account_and_condition(self) -> None:
        captured: dict[str, str] = {}

        def fake_run_psql(url: str, sql: str) -> None:
            captured["sql"] = sql

        with patch.object(run_real_funds_canary_store_truth_cli_preflight, "run_psql", fake_run_psql):
            run_real_funds_canary_store_truth_cli_preflight.seed_runtime_truth(
                "postgres://pmx@localhost/pmx",
                "acct-1",
                "cond-1",
            )
        self.assertIn("account_id, condition_id", captured["sql"])
        self.assertIn("'acct-1', 'cond-1'", captured["sql"])
        self.assertIn("'heartbeat'", captured["sql"])
        self.assertIn("'resource-refresh'", captured["sql"])


if __name__ == "__main__":
    unittest.main()
