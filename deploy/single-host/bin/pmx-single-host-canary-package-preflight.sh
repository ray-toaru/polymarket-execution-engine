#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: pmx-single-host-canary-package-preflight.sh <review-package-dir>" >&2
  exit 2
fi

ROOT="${PMX_EXECUTION_ENGINE_ROOT:-/opt/polymarket-execution-engine}"
REVIEW_DIR="$1"

cd "${ROOT}"

python validation/validate_controlled_canary_external_references.py \
  --file "${REVIEW_DIR}/external-references.json"

python - "${REVIEW_DIR}" <<'PY'
import json
import sys
from pathlib import Path

review_dir = Path(sys.argv[1])
required = [
    "approval.json",
    "external-references.json",
    "release-decision.json",
    "review.json",
]
missing = [name for name in required if not (review_dir / name).exists()]
if missing:
    raise SystemExit(f"review package missing files: {', '.join(missing)}")

approval = json.loads((review_dir / "approval.json").read_text())
decision = json.loads((review_dir / "release-decision.json").read_text())
review = json.loads((review_dir / "review.json").read_text())
external = json.loads((review_dir / "external-references.json").read_text())

failures = []
for label, data in [
    ("approval", approval),
    ("release decision", decision),
    ("review", review),
    ("external references", external),
]:
    text = json.dumps(data, sort_keys=True)
    for token in [
        "-----BEGIN",
        "PRIVATE KEY",
        "clob_secret",
        "raw_signature",
        "raw_signed_payload",
        "signed_order_envelope",
    ]:
        if token in text:
            failures.append(f"{label} contains forbidden sensitive token {token}")

if decision.get("decision") != "no_go":
    failures.append("single-host canary package preflight only accepts no_go release decisions")
for key in [
    "live_submit_authorized",
    "live_cancel_authorized",
    "production_deployment_authorized",
    "real_funds_canary_authorized",
    "remote_side_effects_authorized",
]:
    if decision.get(key) is not False:
        failures.append(f"release decision must keep {key}=false")
for key in [
    "live_submit_allowed",
    "live_cancel_allowed",
    "real_funds_canary_authorized",
    "remote_side_effects",
    "secrets_included",
]:
    if review.get(key) is not False:
        failures.append(f"review must keep {key}=false")
    if key in external and external.get(key) is not False:
        failures.append(f"external references must keep {key}=false")

for key in ["artifact_sha256", "evidence_manifest_sha256"]:
    values = {
        "approval": approval.get(key),
        "decision": decision.get(key),
        "review": review.get(key),
        "external": external.get(key),
    }
    if len(set(values.values())) != 1:
        failures.append(f"{key} mismatch across package: {values}")
    value = next(iter(values.values()))
    if not isinstance(value, str) or len(value) != 64:
        failures.append(f"{key} must be a concrete 64-hex digest")

if review.get("external_references_placeholders_remaining"):
    failures.append("external references placeholders remain")
if external.get("references_only_no_secret_values") is not True:
    failures.append("external references must be reference-only")

if failures:
    raise SystemExit("; ".join(failures))

print("single-host canary package preflight passed; no_go remains enforced")
PY
