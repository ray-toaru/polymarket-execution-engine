#!/usr/bin/env python3
"""Tests for store-truth CLI preflight evidence wiring."""
from __future__ import annotations

import unittest

import check_current_evidence_manifest
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


if __name__ == "__main__":
    unittest.main()
