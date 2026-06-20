#!/usr/bin/env python3
"""Run the reviewed-go canary workflow from preflight through local closeout."""
from __future__ import annotations

import argparse
import json
import os
import subprocess
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
RUN_REVIEWED_GO = INTEGRATION_ROOT / "scripts" / "run_reviewed_go_canary.py"
RUN_REVIEWED_GO_ARMED = INTEGRATION_ROOT / "scripts" / "run_reviewed_go_canary_armed.py"
PREPARE_CLOSEOUT = INTEGRATION_ROOT / "scripts" / "prepare_canary_closeout.py"
ADAPTER_MANIFEST = (
    ROOT
    / "adapters"
    / "pmx-official-sdk-adapter"
    / "Cargo.toml"
)


def resolve(path: Path) -> Path:
    return path if path.is_absolute() else INTEGRATION_ROOT / path


def require_file(path: Path, label: str) -> Path:
    if not path.is_file():
        raise SystemExit(f"{label} missing: {path}")
    return path


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def consumed_markers(package_dir: Path) -> list[Path]:
    return sorted(package_dir.glob("approval-consumed-*.json"))


def parse_env_file(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    for raw_line in path.read_text().splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            raise SystemExit(f"invalid env assignment in {path}: {raw_line}")
        key, value = line.split("=", 1)
        values[key.strip()] = value.strip()
    return values


def combined_runtime_env(env_file: Path, secrets_env_file: Path | None) -> dict[str, str]:
    values = parse_env_file(env_file)
    if secrets_env_file is not None:
        values.update(parse_env_file(require_file(resolve(secrets_env_file), "runtime secrets env")))
    return values


def require_text(data: dict[str, Any], field: str) -> str:
    value = data.get(field)
    if not isinstance(value, str) or not value.strip():
        raise SystemExit(f"{field} is required")
    return value.strip()


def resolve_account_address(runtime_env: dict[str, str], explicit: str | None) -> str:
    if isinstance(explicit, str) and explicit.strip():
        return explicit.strip()
    funder = runtime_env.get("PMX_CLOB_FUNDER", "").strip()
    if funder:
        return funder
    raise SystemExit(
        "account address is required for Data API readback; pass --account-address or provide PMX_CLOB_FUNDER in the runtime env"
    )


def build_workflow_plan(
    *,
    package_dir: Path,
    env_file: Path,
    secrets_env_file: Path | None,
    release_zip: Path | None,
    daily_used_notional_usd: str,
    account_address: str | None,
    include_live_config_overrides: bool,
    readback_closeout_only: bool = False,
) -> dict[str, Any]:
    package_dir = resolve(package_dir)
    env_file = require_file(resolve(env_file), "runtime env")
    release_zip = resolve(release_zip) if release_zip else None

    candidate_market = load_json(require_file(package_dir / "candidate-market.json", "candidate market"))
    approval = load_json(require_file(package_dir / "approval.json", "approval"))
    secrets_env_file = resolve(secrets_env_file) if secrets_env_file is not None else None
    runtime_env = combined_runtime_env(env_file, secrets_env_file)

    token_id = require_text(candidate_market, "token_id")
    market_id = require_text(candidate_market, "market_id")
    remote_account = resolve_account_address(runtime_env, account_address)

    preflight_cmd = [
        "python",
        str(RUN_REVIEWED_GO),
        "--package-dir",
        str(package_dir),
        "--env-file",
        str(env_file),
    ]
    if secrets_env_file is not None:
        preflight_cmd.extend(["--secrets-env-file", str(secrets_env_file)])
    preflight_cmd.extend(
        [
            "--mode",
            "preflight",
            "--daily-used-notional-usd",
            daily_used_notional_usd,
            "--run",
        ]
    )
    armed_cmd = [
        "python",
        str(RUN_REVIEWED_GO_ARMED),
        "--package-dir",
        str(package_dir),
        "--env-file",
        str(env_file),
    ]
    if secrets_env_file is not None:
        armed_cmd.extend(["--secrets-env-file", str(secrets_env_file)])
    armed_cmd.extend(
        [
            "--daily-used-notional-usd",
            daily_used_notional_usd,
            "--run",
        ]
    )
    order_query_output = package_dir / "order-status-query.json"
    order_query_cmd = [
        "cargo",
        "run",
        "--manifest-path",
        str(ADAPTER_MANIFEST),
        "--features",
        "live-submit",
        "--bin",
        "pmx-query-order",
        "--",
        "--order-id",
        "__REMOTE_ORDER_ID__",
    ]
    trade_query_output = package_dir / "trade-fill-query.json"
    trade_query_cmd = [
        "cargo",
        "run",
        "--manifest-path",
        str(ADAPTER_MANIFEST),
        "--features",
        "live-submit",
        "--bin",
        "pmx-query-trades",
        "--",
        "--token-id",
        token_id,
        "--order-id",
        "__REMOTE_ORDER_ID__",
    ]
    activity_query_output = package_dir / "account-activity-readback.json"
    activity_query_cmd = [
        "cargo",
        "run",
        "--manifest-path",
        str(ADAPTER_MANIFEST),
        "--features",
        "data-readback",
        "--bin",
        "pmx-query-account-activity",
        "--",
        "--account",
        remote_account,
        "--market-id",
        market_id,
        "--token-id",
        token_id,
    ]
    closeout_cmd = [
        "python",
        str(PREPARE_CLOSEOUT),
        "--package-dir",
        str(package_dir),
    ]
    if release_zip is not None:
        closeout_cmd.extend(["--release-zip", str(release_zip)])

    readback_steps = [
        {"name": "order_query", "command": order_query_cmd, "stdout_path": str(order_query_output)},
        {"name": "trade_query", "command": trade_query_cmd, "stdout_path": str(trade_query_output)},
        {
            "name": "account_activity_query",
            "command": activity_query_cmd,
            "stdout_path": str(activity_query_output),
        },
        {"name": "closeout", "command": closeout_cmd},
    ]

    if readback_closeout_only:
        markers = consumed_markers(package_dir)
        if not markers:
            raise SystemExit("readback-closeout-only requires an existing approval-consumed marker")
        remote_order_id = parse_remote_order_id("", package_dir)
        return {
            "status": "ready",
            "mode": "readback_closeout_only",
            "package_dir": str(package_dir),
            "env_file": str(env_file),
            "secrets_env_file": str(secrets_env_file) if secrets_env_file is not None else None,
            "account_id": require_text(approval, "account_id"),
            "account_address": remote_account,
            "market_id": market_id,
            "token_id": token_id,
            "daily_used_notional_usd": daily_used_notional_usd,
            "includes_live_config_overrides": False,
            "uses_dedicated_armed_wrapper": True,
            "execution_plane": "polymarket-execution-engine/validation/run_reviewed_go_canary_closeout.py",
            "will_submit": False,
            "will_cancel": False,
            "remote_side_effects": False,
            "remote_order_id": remote_order_id,
            "approval_consumed_markers": [str(path) for path in markers],
            "steps": readback_steps,
        }

    return {
        "status": "ready",
        "mode": "full_canary_workflow",
        "package_dir": str(package_dir),
        "env_file": str(env_file),
        "secrets_env_file": str(secrets_env_file) if secrets_env_file is not None else None,
        "account_id": require_text(approval, "account_id"),
        "account_address": remote_account,
        "market_id": market_id,
        "token_id": token_id,
        "daily_used_notional_usd": daily_used_notional_usd,
        "includes_live_config_overrides": False,
        "uses_dedicated_armed_wrapper": True,
        "execution_plane": "polymarket-execution-engine/validation/run_reviewed_go_canary_closeout.py",
        "will_submit": True,
        "will_cancel": True,
        "steps": [
            {"name": "preflight", "command": preflight_cmd},
            {"name": "armed", "command": armed_cmd},
            *readback_steps,
        ],
    }


def run_step(
    *,
    name: str,
    command: list[str],
    env: dict[str, str],
    stdout_path: Path | None = None,
) -> subprocess.CompletedProcess[str]:
    if stdout_path is None:
        return subprocess.run(command, cwd=INTEGRATION_ROOT, text=True, env=env, check=False)
    stdout_path.parent.mkdir(parents=True, exist_ok=True)
    with stdout_path.open("w") as fh:
        return subprocess.run(command, cwd=INTEGRATION_ROOT, text=True, env=env, stdout=fh, check=False)


def parse_remote_order_id(armed_stdout: str, package_dir: Path) -> str:
    report_path = package_dir / "post-canary-report.json"
    if report_path.exists():
        report = load_json(report_path)
        value = report.get("remote_order_id")
        if isinstance(value, str) and value.strip():
            return value.strip()
    try:
        payload = json.loads(armed_stdout)
    except json.JSONDecodeError:
        payload = None
    if isinstance(payload, dict):
        value = payload.get("remote_order_id")
        if isinstance(value, str) and value.strip():
            return value.strip()
    raise SystemExit("armed canary did not produce a remote_order_id")


def execute_workflow(plan: dict[str, Any]) -> dict[str, Any]:
    secrets_env_file = Path(plan["secrets_env_file"]) if plan.get("secrets_env_file") else None
    runtime_env = combined_runtime_env(Path(plan["env_file"]), secrets_env_file)
    env = dict(os.environ)
    env.update(runtime_env)
    package_dir = Path(plan["package_dir"])
    steps = plan["steps"]
    results: list[dict[str, Any]] = []

    if plan.get("mode") == "readback_closeout_only":
        remote_order_id = parse_remote_order_id("", package_dir)
        for step in steps[:3]:
            command = [remote_order_id if token == "__REMOTE_ORDER_ID__" else token for token in step["command"]]
            completed = run_step(
                name=step["name"],
                command=command,
                env=env,
                stdout_path=Path(step["stdout_path"]),
            )
            results.append({"name": step["name"], "returncode": completed.returncode, "stdout_path": step["stdout_path"]})
            if completed.returncode != 0:
                raise SystemExit(f"{step['name']} failed")

        closeout = run_step(name="closeout", command=steps[3]["command"], env=env)
        results.append({"name": "closeout", "returncode": closeout.returncode})
        if closeout.returncode != 0:
            raise SystemExit(closeout.stderr or closeout.stdout or "closeout step failed")

        return {
            "status": "completed",
            "remote_order_id": remote_order_id,
            "package_dir": plan["package_dir"],
            "results": results,
            "closeout_json": str(package_dir / "closeout.json"),
            "closeout_md": str(package_dir / "CLOSEOUT.md"),
        }

    if consumed_markers(package_dir) or (package_dir / "post-canary-report.json").exists():
        raise SystemExit(
            "refusing to run full canary workflow for a package with existing post-canary evidence; "
            "use --readback-closeout-only to avoid a second armed attempt"
        )

    preflight = run_step(name="preflight", command=steps[0]["command"], env=env)
    results.append({"name": "preflight", "returncode": preflight.returncode})
    if preflight.returncode != 0:
        raise SystemExit(preflight.stderr or preflight.stdout or "preflight step failed")

    armed = run_step(name="armed", command=steps[1]["command"], env=env)
    results.append({"name": "armed", "returncode": armed.returncode})
    if armed.returncode != 0:
        raise SystemExit(armed.stderr or armed.stdout or "armed step failed")
    remote_order_id = parse_remote_order_id(armed.stdout, package_dir)

    for step in steps[2:5]:
        command = [remote_order_id if token == "__REMOTE_ORDER_ID__" else token for token in step["command"]]
        completed = run_step(
            name=step["name"],
            command=command,
            env=env,
            stdout_path=Path(step["stdout_path"]),
        )
        results.append({"name": step["name"], "returncode": completed.returncode, "stdout_path": step["stdout_path"]})
        if completed.returncode != 0:
            raise SystemExit(f"{step['name']} failed")

    closeout = run_step(name="closeout", command=steps[5]["command"], env=env)
    results.append({"name": "closeout", "returncode": closeout.returncode})
    if closeout.returncode != 0:
        raise SystemExit(closeout.stderr or closeout.stdout or "closeout step failed")

    return {
        "status": "completed",
        "remote_order_id": remote_order_id,
        "package_dir": plan["package_dir"],
        "results": results,
        "closeout_json": str(package_dir / "closeout.json"),
        "closeout_md": str(package_dir / "CLOSEOUT.md"),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package-dir", required=True, type=Path)
    parser.add_argument("--env-file", required=True, type=Path)
    parser.add_argument("--secrets-env-file", type=Path)
    parser.add_argument("--release-zip", type=Path)
    parser.add_argument("--daily-used-notional-usd", default="0")
    parser.add_argument("--account-address")
    parser.add_argument(
        "--include-live-config-overrides",
        action="store_true",
        help=(
            "Accepted for wrapper compatibility. Dedicated armed and preflight wrappers already enforce the live-config boundary."
        ),
    )
    parser.add_argument(
        "--run",
        action="store_true",
        help="Execute preflight, armed canary, readback queries, and local closeout. Without this flag the script only prints the workflow plan.",
    )
    parser.add_argument(
        "--readback-closeout-only",
        action="store_true",
        help=(
            "Only run post-attempt readback queries and local closeout for an already consumed canary package. "
            "This mode never runs preflight or the armed submit/cancel step."
        ),
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    plan = build_workflow_plan(
        package_dir=args.package_dir,
        env_file=args.env_file,
        secrets_env_file=args.secrets_env_file,
        release_zip=args.release_zip,
        daily_used_notional_usd=args.daily_used_notional_usd,
        account_address=args.account_address,
        include_live_config_overrides=args.include_live_config_overrides,
        readback_closeout_only=args.readback_closeout_only,
    )
    if not args.run:
        print(json.dumps(plan, indent=2, sort_keys=True))
        return 0
    result = execute_workflow(plan)
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
