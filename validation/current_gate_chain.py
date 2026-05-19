"""Helpers for validating the current release gate chain."""
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CURRENT_GATES = ROOT / "validation" / "run_current_gates.sh"
ACTIVE_GATE_IMPLEMENTATION = ROOT / "validation" / "run_current_gates_impl.sh"


def current_gate_text() -> str:
    return CURRENT_GATES.read_text()


def active_gate_implementation_text() -> str:
    return ACTIVE_GATE_IMPLEMENTATION.read_text()


def require_current_gate_log(log_name: str, description: str, failures: list[str]) -> None:
    current_gates = current_gate_text()
    implementation_gates = active_gate_implementation_text()
    if ACTIVE_GATE_IMPLEMENTATION.name not in current_gates:
        failures.append("run_current_gates.sh must delegate to the active gate implementation")
    if log_name not in implementation_gates:
        failures.append(f"current gates must emit {description} log")
