#!/usr/bin/env bash
set -euo pipefail

echo "single-host rollback: forcing fail-closed operator state"
export PMX_LIVE_SUBMIT_ENABLED=0
export PMX_LIVE_CANCEL_ENABLED=0
export PMX_PRODUCTION_DEPLOYMENT_ENABLED=0
export PMX_ALLOW_LIVE_SUBMIT=0
export PMX_ALLOW_LIVE_CANCEL=0
export PMX_ALLOW_REAL_FUNDS_CANARY=0
export PMX_KILL_SWITCH_OPEN=0

systemctl stop 'pmx-real-funds-canary@*.service' 2>/dev/null || true
systemctl restart pmx-api.service

echo "single-host rollback requested; verify runtime, reconcile, and audit evidence before any further action"
