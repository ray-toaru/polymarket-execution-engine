#!/usr/bin/env python3
"""Prepare or run the privileged armed reviewed-go canary invocation."""
from __future__ import annotations

import argparse
import importlib.util
import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
BASE_SCRIPT = INTEGRATION_ROOT / "scripts" / "run_reviewed_go_canary.py"


def load_base():
    spec = importlib.util.spec_from_file_location("run_reviewed_go_canary_base", BASE_SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package-dir", required=True, type=Path)
    parser.add_argument("--env-file", required=True, type=Path)
    parser.add_argument("--daily-used-notional-usd", default="0")
    parser.add_argument("--idempotency-key")
    parser.add_argument("--execution-id")
    parser.add_argument("--plan-hash")
    parser.add_argument("--report-file", type=Path)
    parser.add_argument("--approval-consumed-marker", type=Path)
    parser.add_argument(
        "--run",
        action="store_true",
        help="Execute the privileged armed cargo command. Without this flag the script only prints the invocation plan.",
    )
    return parser.parse_args()


def build_armed_invocation(
    *,
    package_dir: Path,
    env_file: Path,
    daily_used_notional_usd: str,
    idempotency_key: str | None,
    execution_id: str | None,
    plan_hash: str | None,
    report_file: Path | None,
    approval_consumed_marker: Path | None,
) -> dict[str, object]:
    base = load_base()
    invocation = base.build_invocation(
        package_dir=base.resolve(package_dir),
        env_file=base.resolve(env_file),
        mode="armed",
        daily_used_notional_usd=daily_used_notional_usd,
        idempotency_key=idempotency_key,
        execution_id=execution_id,
        plan_hash=plan_hash,
        report_file=base.resolve(report_file) if report_file else None,
        approval_consumed_marker=base.resolve(approval_consumed_marker)
        if approval_consumed_marker
        else None,
        include_live_config_overrides=True,
    )
    invocation["wrapper"] = "run_reviewed_go_canary_armed.py"
    invocation["armed_wrapper"] = True
    invocation["requires_explicit_live_config_overrides"] = False
    return invocation


def main() -> int:
    args = parse_args()
    base = load_base()
    invocation = build_armed_invocation(
        package_dir=args.package_dir,
        env_file=args.env_file,
        daily_used_notional_usd=args.daily_used_notional_usd,
        idempotency_key=args.idempotency_key,
        execution_id=args.execution_id,
        plan_hash=args.plan_hash,
        report_file=args.report_file,
        approval_consumed_marker=args.approval_consumed_marker,
    )
    if not args.run:
        print(json.dumps(invocation, indent=2, sort_keys=True))
        return 0

    missing = invocation["missing_gate_env_vars"]
    if missing:
        raise SystemExit(
            "cannot execute reviewed-go armed canary; missing required gate env vars: "
            + ", ".join(missing)
        )

    completed = subprocess.run(
        invocation["command"],
        cwd=INTEGRATION_ROOT,
        text=True,
        check=False,
    )
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
