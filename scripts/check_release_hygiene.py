#!/usr/bin/env python3
from __future__ import annotations

import sys
import zipfile
from pathlib import Path

FORBIDDEN_PARTS = {"__pycache__", ".pytest_cache", ".mypy_cache", "target"}
FORBIDDEN_SUFFIXES = {".pyc", ".pyo", ".sqlite", ".sqlite3", ".db"}
FORBIDDEN_FILENAMES = {".env"}


def forbidden(path: str) -> bool:
    parts = tuple(Path(path).parts)
    name = parts[-1] if parts else path
    suffix = Path(name).suffix
    return (
        any(part in FORBIDDEN_PARTS for part in parts)
        or suffix in FORBIDDEN_SUFFIXES
        or name in FORBIDDEN_FILENAMES
    )


def scan_directory(root: Path) -> tuple[str, list[str]]:
    problems: list[str] = []
    for path in root.rglob("*"):
        rel = path.relative_to(root)
        if forbidden(str(rel)):
            problems.append(str(rel))
    return "directory", problems


def scan_zip(root: Path) -> tuple[str, list[str]]:
    problems: list[str] = []
    with zipfile.ZipFile(root) as zf:
        for member in zf.namelist():
            if forbidden(member):
                problems.append(member)
    return "zip", problems


def main() -> int:
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.cwd()
    if root.is_file() and root.suffix == ".zip":
        mode, problems = scan_zip(root)
    else:
        mode, problems = scan_directory(root)
    if problems:
        print(f"release hygiene failed mode={mode}:")
        for item in sorted(set(problems)):
            print(f" - {item}")
        return 1
    print(f"release hygiene passed mode={mode}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
