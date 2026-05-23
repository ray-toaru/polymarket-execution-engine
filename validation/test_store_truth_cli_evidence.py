#!/usr/bin/env python3
"""Tests for store-truth CLI preflight evidence wiring."""
from __future__ import annotations

import unittest

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
        self.assertFalse(doc["live_submit_allowed"])
        self.assertFalse(doc["remote_side_effects"])


if __name__ == "__main__":
    unittest.main()
