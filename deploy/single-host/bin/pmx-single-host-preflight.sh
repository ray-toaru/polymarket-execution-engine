#!/usr/bin/env bash
set -euo pipefail

ROOT="${PMX_EXECUTION_ENGINE_ROOT:-/opt/polymarket-execution-engine}"
cd "${ROOT}"

if [[ "${PMX_LIVE_SUBMIT_ENABLED:-0}" == "1" || "${PMX_ALLOW_LIVE_SUBMIT:-0}" == "1" ]]; then
  echo "single-host preflight refused: live submit is enabled" >&2
  exit 1
fi

if [[ "${PMX_LIVE_CANCEL_ENABLED:-0}" == "1" || "${PMX_ALLOW_LIVE_CANCEL:-0}" == "1" ]]; then
  echo "single-host preflight refused: live cancel is enabled" >&2
  exit 1
fi

if [[ "${PMX_PRODUCTION_DEPLOYMENT_ENABLED:-0}" == "1" ]]; then
  echo "single-host preflight refused: production deployment is enabled" >&2
  exit 1
fi

if [[ -z "${PM_EXEC_SERVICE_TOKEN:-}" || -z "${PM_EXEC_ADMIN_TOKEN:-}" ]]; then
  echo "single-host preflight refused: PM_EXEC_SERVICE_TOKEN and PM_EXEC_ADMIN_TOKEN are required" >&2
  exit 1
fi

if [[ "${PM_EXEC_SERVICE_TOKEN}" == "${PM_EXEC_ADMIN_TOKEN}" ]]; then
  echo "single-host preflight refused: service/admin tokens must be distinct" >&2
  exit 1
fi

if [[ "${PMX_API_STORAGE:-postgres}" != "postgres" ]]; then
  echo "single-host preflight refused: PMX_API_STORAGE must be postgres" >&2
  exit 1
fi

python validation/check_live_submit_guard.py
python validation/check_production_readiness_guard.py
python validation/check_docs_evidence_governance.py

echo "single-host limited preflight passed; live submit/cancel remain disabled"
