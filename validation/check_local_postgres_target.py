#!/usr/bin/env python3
"""Inspect the local PostgreSQL target referenced by .env without mutating system state."""
from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path
from typing import Any
from urllib.parse import urlparse


ROOT = Path(__file__).resolve().parents[1]


def load_env_file(path: Path) -> None:
    if not path.exists():
        return
    for raw_line in path.read_text().splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        value = value.strip().strip("'").strip('"')
        if key and key not in os.environ:
            os.environ[key] = value


def database_url() -> str:
    load_env_file(ROOT / ".env")
    url = os.environ.get("PMX_TEST_DATABASE_URL") or os.environ.get("PMX_DATABASE_URL")
    if not url or not url.strip():
        raise SystemExit("PMX_TEST_DATABASE_URL or PMX_DATABASE_URL is required")
    return url


def parse_database_target(url: str) -> dict[str, Any]:
    parsed = urlparse(url)
    default_port = 5432 if parsed.scheme.startswith("postgres") else None
    return {
        "scheme": parsed.scheme or "unknown",
        "hostname": parsed.hostname or "unknown",
        "port": parsed.port or default_port,
        "database": parsed.path.lstrip("/") or "unknown",
        "username": parsed.username or "<none>",
    }


def pg_lsclusters() -> list[dict[str, str]]:
    result = subprocess.run(
        ["pg_lsclusters"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(
            json.dumps(
                {
                    "status": "fail",
                    "stage": "pg_lsclusters",
                    "returncode": result.returncode,
                    "stderr": result.stderr.strip() or "<empty>",
                },
                indent=2,
                sort_keys=True,
            )
        )
    lines = [line for line in result.stdout.splitlines() if line.strip()]
    if len(lines) < 2:
        return []
    clusters: list[dict[str, str]] = []
    for line in lines[1:]:
        parts = line.split()
        if len(parts) < 6:
            continue
        clusters.append(
            {
                "version": parts[0],
                "cluster": parts[1],
                "port": parts[2],
                "status": parts[3],
                "owner": parts[4],
                "data_directory": parts[5],
                "log_file": parts[6] if len(parts) > 6 else "",
            }
        )
    return clusters


def matching_cluster(target: dict[str, Any], clusters: list[dict[str, str]]) -> dict[str, str] | None:
    port = str(target.get("port") or "")
    for cluster in clusters:
        if cluster.get("port") == port:
            return cluster
    return None


def pg_isready(target: dict[str, Any]) -> dict[str, Any]:
    cmd = [
        "pg_isready",
        "-h",
        str(target["hostname"]),
        "-p",
        str(target["port"]),
        "-d",
        str(target["database"]),
    ]
    username = target.get("username")
    if username and username != "<none>":
        cmd.extend(["-U", str(username)])
    result = subprocess.run(
        cmd,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    return {
        "returncode": result.returncode,
        "stdout": result.stdout.strip() or "<empty>",
        "stderr": result.stderr.strip() or "<empty>",
    }


def recommendations(target: dict[str, Any], cluster: dict[str, str] | None, readiness: dict[str, Any]) -> list[str]:
    actions: list[str] = []
    if cluster is None:
        actions.append(
            "No local PostgreSQL cluster matches the configured port; align .env with a real listener or create a cluster."
        )
        return actions
    if cluster.get("status") != "online":
        actions.append(
            f"Configured cluster {cluster['version']}/{cluster['cluster']} is down; start it as root or postgres: pg_ctlcluster {cluster['version']} {cluster['cluster']} start"
        )
    if readiness.get("returncode") != 0:
        actions.append(
            "pg_isready still fails for the configured endpoint; verify host, port, database, user, and pg_hba.conf access."
        )
    if not actions:
        actions.append("Local PostgreSQL target is reachable and matches the configured .env endpoint.")
    return actions


def inspect() -> dict[str, Any]:
    url = database_url()
    target = parse_database_target(url)
    clusters = pg_lsclusters()
    cluster = matching_cluster(target, clusters)
    readiness = pg_isready(target)
    ok = cluster is not None and cluster.get("status") == "online" and readiness.get("returncode") == 0
    return {
        "status": "pass" if ok else "fail",
        "database_target": target,
        "matched_cluster": cluster,
        "pg_isready": readiness,
        "recommendations": recommendations(target, cluster, readiness),
    }


def main() -> int:
    result = inspect()
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if result["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
