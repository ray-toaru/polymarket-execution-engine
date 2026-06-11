from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path
from zipfile import ZipFile


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "validation" / "check_release_artifact.py"


def load_module():
    spec = importlib.util.spec_from_file_location("check_release_artifact", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class ReleaseArtifactSecretScanTests(unittest.TestCase):
    def setUp(self) -> None:
        self.module = load_module()

    def test_detects_private_key_and_clob_secret_assignments(self) -> None:
        samples = [
            b"-----BEGIN PRIVATE KEY-----",
            b"POLYMARKET_PRIVATE_KEY=0xdeadbeef",
            b"POLY_API_SECRET=not-a-real-secret",
            b"POLY_API_PASSPHRASE=not-a-real-passphrase",
        ]
        for sample in samples:
            with self.subTest(sample=sample):
                self.assertTrue(self.module.contains_forbidden_secret_content(sample))

    def test_allows_only_named_redaction_fixture_with_required_markers(self) -> None:
        member = (
            "release-root/polymarket-execution-engine/adapters/"
            "pmx-official-sdk-adapter/src/tests/liveness_errors.rs"
        )
        fixture = (
            b"fn redacts_named_secret_assignments() { "
            b"redact_sensitive_text(\"POLY_API_SECRET=value\"); "
            b"assert_eq!(result, \"[REDACTED]\"); }"
        )
        self.assertTrue(
            self.module.allowed_secret_content_test_fixture(
                member,
                "release-root",
                fixture,
            )
        )
        self.assertFalse(
            self.module.allowed_secret_content_test_fixture(
                member,
                "release-root",
                b"POLY_API_SECRET=value",
            )
        )
        self.assertFalse(
            self.module.allowed_secret_content_test_fixture(
                "release-root/docs/example.md",
                "release-root",
                fixture,
            )
        )

    def test_archive_validation_rejects_secret_like_content(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_name:
            archive = Path(tmp_name) / "release.zip"
            with ZipFile(archive, "w") as zf:
                zf.writestr("release-root/VERSION", "0.1.0\n")
                zf.writestr(
                    "release-root/docs/leak.txt",
                    "POLY_API_SECRET=not-a-real-secret\n",
                )
            with ZipFile(archive) as zf:
                failures = self.module.validate_archive_members(
                    zf,
                    expected_root="release-root",
                    expected_version="0.1.0",
                )
        self.assertTrue(
            any("forbidden secret-like content" in failure for failure in failures)
        )


if __name__ == "__main__":
    unittest.main()
