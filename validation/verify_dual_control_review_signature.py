#!/usr/bin/env python3
"""Verify offline review signatures against a reviewer registry."""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable


RunCommand = Callable[..., subprocess.CompletedProcess[str]]


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def parse_time(value: object, label: str) -> datetime | None:
    if value in (None, ""):
        return None
    if not isinstance(value, str):
        raise SystemExit(f"{label} must be an RFC3339 timestamp")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise SystemExit(f"{label} must be an RFC3339 timestamp") from exc
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


def require_text(value: object, label: str) -> str:
    if not isinstance(value, str) or not value.strip() or value.startswith("REPLACE_WITH_"):
        raise SystemExit(f"{label} must be concrete")
    return value.strip()


def resolve_relative(path: str, base_file: Path) -> Path:
    candidate = Path(path)
    return candidate if candidate.is_absolute() else base_file.parent / candidate


def reviewer_record(registry: dict[str, Any], identity_ref: str) -> dict[str, Any]:
    if registry.get("schema_version") != 1:
        raise SystemExit("reviewer registry schema_version must be 1")
    reviewers = registry.get("reviewers")
    if not isinstance(reviewers, list):
        raise SystemExit("reviewer registry reviewers must be a list")
    matches = [item for item in reviewers if isinstance(item, dict) and item.get("reviewer_identity_ref") == identity_ref]
    if len(matches) != 1:
        raise SystemExit("reviewer registry must contain exactly one matching reviewer_identity_ref")
    return matches[0]


def reviewer_record_by_ssh_principal(registry: dict[str, Any], principal: str) -> dict[str, Any]:
    if registry.get("schema_version") != 1:
        raise SystemExit("reviewer registry schema_version must be 1")
    reviewers = registry.get("reviewers")
    if not isinstance(reviewers, list):
        raise SystemExit("reviewer registry reviewers must be a list")
    matches = [
        item
        for item in reviewers
        if isinstance(item, dict) and item.get("ssh_principal") == principal
    ]
    if len(matches) != 1:
        raise SystemExit("reviewer registry must contain exactly one matching reviewer ssh_principal")
    return matches[0]


def review_identity_and_record(
    review: dict[str, Any],
    registry: dict[str, Any],
) -> tuple[str, dict[str, Any], str | None, bool]:
    legacy_identity = review.get("reviewer_identity_ref")
    if legacy_identity not in (None, ""):
        identity = require_text(legacy_identity, "dual-control review reviewer_identity_ref")
        return identity, reviewer_record(registry, identity), None, True

    reviewer = review.get("reviewer")
    if not isinstance(reviewer, dict):
        raise SystemExit("review must include reviewer_identity_ref or reviewer.identity_ref")
    signed_principal = require_text(
        reviewer.get("identity_ref"),
        "review reviewer.identity_ref",
    )
    record = reviewer_record_by_ssh_principal(registry, signed_principal)
    identity = require_text(
        record.get("reviewer_identity_ref"),
        "reviewer registry reviewer_identity_ref",
    )
    return identity, record, signed_principal, False


def validate_reviewer_record(record: dict[str, Any], *, now: datetime) -> None:
    status = require_text(record.get("status"), "reviewer registry status")
    if status != "active":
        raise SystemExit(f"reviewer registry status must be active, got {status}")
    if parse_time(record.get("revoked_at"), "reviewer registry revoked_at") is not None:
        raise SystemExit("reviewer registry entry is revoked")
    expires_at = parse_time(record.get("expires_at"), "reviewer registry expires_at")
    if expires_at is not None and expires_at <= now:
        raise SystemExit("reviewer registry entry is expired")
    method = require_text(record.get("allowed_signing_method"), "reviewer registry allowed_signing_method")
    if method not in {"gpg", "ssh"}:
        raise SystemExit("reviewer registry allowed_signing_method must be gpg or ssh")


def validate_canonical_review(review_path: Path, canonical_path: Path) -> None:
    review = load_json(review_path)
    canonical = load_json(canonical_path)
    if canonical != review:
        raise SystemExit("canonical dual-control review does not match approved review JSON")


def validate_signature_evidence(review: dict[str, Any], record: dict[str, Any], registry_path: Path) -> None:
    evidence_ref = require_text(
        record.get("signing_key_attestation_file"),
        "reviewer registry signing_key_attestation_file",
    )
    evidence_path = resolve_relative(evidence_ref, registry_path)
    if not evidence_path.is_file():
        raise SystemExit(f"reviewer signing-key attestation missing: {evidence_path}")
    expected_sha = require_text(
        review.get("review_signature_evidence_sha256"),
        "dual-control review review_signature_evidence_sha256",
    ).lower()
    actual_sha = sha256(evidence_path)
    if actual_sha != expected_sha:
        raise SystemExit("review_signature_evidence_sha256 does not match signing-key attestation")
    attestation = load_json(evidence_path)
    if attestation.get("reviewer_identity_ref") != review.get("reviewer_identity_ref"):
        raise SystemExit("signing-key attestation reviewer_identity_ref mismatch")
    if attestation.get("signing_method") != record.get("allowed_signing_method"):
        raise SystemExit("signing-key attestation signing_method mismatch")
    fingerprint = record.get("signing_key_fingerprint")
    if fingerprint and attestation.get("signing_key_fingerprint") != fingerprint:
        raise SystemExit("signing-key attestation fingerprint mismatch")


def validate_optional_signature_evidence(
    review: dict[str, Any],
    record: dict[str, Any],
    registry_path: Path,
) -> None:
    evidence_ref = record.get("signing_key_attestation_file")
    if not evidence_ref:
        return
    evidence_path = resolve_relative(
        require_text(evidence_ref, "reviewer registry signing_key_attestation_file"),
        registry_path,
    )
    if not evidence_path.is_file():
        raise SystemExit(f"reviewer signing-key attestation missing: {evidence_path}")
    attestation = load_json(evidence_path)
    if attestation.get("reviewer_identity_ref") != record.get("reviewer_identity_ref"):
        raise SystemExit("signing-key attestation reviewer_identity_ref mismatch")
    if attestation.get("signing_method") != record.get("allowed_signing_method"):
        raise SystemExit("signing-key attestation signing_method mismatch")
    fingerprint = record.get("signing_key_fingerprint")
    if fingerprint and attestation.get("signing_key_fingerprint") != fingerprint:
        raise SystemExit("signing-key attestation fingerprint mismatch")


def run_checked(
    cmd: list[str],
    *,
    label: str,
    run_command: RunCommand = subprocess.run,
    **kwargs: Any,
) -> subprocess.CompletedProcess[str]:
    completed = run_command(cmd, text=True, capture_output=True, **kwargs)
    if completed.returncode != 0:
        raise SystemExit(f"{label} failed: {completed.stderr.strip() or completed.stdout.strip()}")
    return completed


def verify_gpg_signature(
    *,
    record: dict[str, Any],
    registry_path: Path,
    canonical_review: Path,
    signature_file: Path,
    run_command: RunCommand = subprocess.run,
) -> dict[str, str]:
    public_key = resolve_relative(require_text(record.get("public_key_file"), "reviewer registry public_key_file"), registry_path)
    if not public_key.is_file():
        raise SystemExit(f"reviewer public key missing: {public_key}")
    expected_fingerprint = require_text(
        record.get("signing_key_fingerprint"),
        "reviewer registry signing_key_fingerprint",
    ).replace(" ", "").upper()
    with tempfile.TemporaryDirectory(prefix="pmx-review-gpg-") as gnupg_home:
        run_checked(
            ["gpg", "--homedir", gnupg_home, "--batch", "--import", str(public_key)],
            label="gpg public key import",
            run_command=run_command,
        )
        verify = run_checked(
            [
                "gpg",
                "--homedir",
                gnupg_home,
                "--batch",
                "--status-fd",
                "1",
                "--verify",
                str(signature_file),
                str(canonical_review),
            ],
            label="gpg signature verification",
            run_command=run_command,
        )
    status_output = verify.stdout + "\n" + verify.stderr
    if expected_fingerprint not in status_output.replace(" ", "").upper():
        raise SystemExit("gpg signature was not made by the registered fingerprint")
    return {"method": "gpg", "fingerprint": expected_fingerprint}


def verify_ssh_signature(
    *,
    record: dict[str, Any],
    registry_path: Path,
    canonical_review: Path,
    signature_file: Path,
    run_command: RunCommand = subprocess.run,
) -> dict[str, str]:
    allowed_signers = resolve_relative(
        require_text(record.get("allowed_signers_file"), "reviewer registry allowed_signers_file"),
        registry_path,
    )
    if not allowed_signers.is_file():
        raise SystemExit(f"reviewer allowed signers file missing: {allowed_signers}")
    principal = require_text(record.get("ssh_principal"), "reviewer registry ssh_principal")
    run_checked(
        [
            "ssh-keygen",
            "-Y",
            "verify",
            "-f",
            str(allowed_signers),
            "-I",
            principal,
            "-n",
            "pmx-canary-review",
            "-s",
            str(signature_file),
        ],
        label="ssh signature verification",
        run_command=run_command,
        input=canonical_review.read_text(),
    )
    return {"method": "ssh", "principal": principal}


def verify_review_signature(
    *,
    approved_review_file: Path,
    canonical_review_file: Path,
    signature_file: Path,
    reviewer_registry_file: Path,
    run_command: RunCommand = subprocess.run,
    now: datetime | None = None,
) -> dict[str, str]:
    if not signature_file.is_file():
        raise SystemExit(f"review signature file missing: {signature_file}")
    validate_canonical_review(approved_review_file, canonical_review_file)
    review = load_json(approved_review_file)
    registry = load_json(reviewer_registry_file)
    identity, record, signed_principal, requires_bound_evidence = review_identity_and_record(review, registry)
    validate_reviewer_record(record, now=now or datetime.now(timezone.utc))
    if requires_bound_evidence:
        validate_signature_evidence(review, record, reviewer_registry_file)
    else:
        validate_optional_signature_evidence(review, record, reviewer_registry_file)
    method = record["allowed_signing_method"]
    if method == "gpg":
        result = verify_gpg_signature(
            record=record,
            registry_path=reviewer_registry_file,
            canonical_review=canonical_review_file,
            signature_file=signature_file,
            run_command=run_command,
        )
    else:
        result = verify_ssh_signature(
            record=record,
            registry_path=reviewer_registry_file,
            canonical_review=canonical_review_file,
            signature_file=signature_file,
            run_command=run_command,
        )
    return {
        "status": "pass",
        "reviewer_identity_ref": identity,
        "signature_method": result["method"],
        "canonical_review_sha256": sha256(canonical_review_file),
        "signature_file_sha256": sha256(signature_file),
    } | (
        {"signed_reviewer_principal": signed_principal}
        if signed_principal is not None
        else {}
    ) | {k: v for k, v in result.items() if k != "method"}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--approved-dual-control-review-file", required=True, type=Path)
    parser.add_argument("--canonical-dual-control-review-file", required=True, type=Path)
    parser.add_argument("--review-signature-file", required=True, type=Path)
    parser.add_argument("--reviewer-registry-file", required=True, type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    result = verify_review_signature(
        approved_review_file=args.approved_dual_control_review_file,
        canonical_review_file=args.canonical_dual_control_review_file,
        signature_file=args.review_signature_file,
        reviewer_registry_file=args.reviewer_registry_file,
    )
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
