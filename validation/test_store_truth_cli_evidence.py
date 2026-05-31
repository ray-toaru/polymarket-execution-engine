#!/usr/bin/env python3
"""Tests for store-truth CLI preflight evidence wiring."""
from __future__ import annotations

import json
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


if __name__ == "__main__":
    unittest.main()
