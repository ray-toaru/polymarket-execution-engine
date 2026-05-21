#!/usr/bin/env python3
from __future__ import annotations

import sys
import zipfile
from pathlib import Path

FORBIDDEN_PARTS = {".venv", "venv", "__pycache__", ".pytest_cache", ".mypy_cache", "target"}
FORBIDDEN_SUFFIXES = {".pyc", ".pyo", ".sqlite", ".sqlite3", ".db"}
FORBIDDEN_FILENAMES = {".env"}
DEV_WORKTREE_ALLOWED_FILENAMES = {".env"}
DEV_WORKTREE_ALLOWED_ROOT_DIRS = {".venv", "venv"}


def forbidden(path: str, *, dev_worktree: bool = False) -> bool:
    parts = tuple(Path(path).parts)
    name = parts[-1] if parts else path
    suffix = Path(name).suffix
    forbidden_filenames = FORBIDDEN_FILENAMES
    if dev_worktree:
        forbidden_filenames = forbidden_filenames - DEV_WORKTREE_ALLOWED_FILENAMES
        if parts and parts[0] in DEV_WORKTREE_ALLOWED_ROOT_DIRS:
            return False
    return (
        any(part in FORBIDDEN_PARTS for part in parts)
        or any(part.endswith(".egg-info") for part in parts)
        or suffix in FORBIDDEN_SUFFIXES
        or name in forbidden_filenames
    )


def scan_directory(root: Path, *, dev_worktree: bool = False) -> tuple[str, list[str]]:
    problems: list[str] = []
    for path in root.rglob("*"):
        rel = path.relative_to(root)
        if forbidden(str(rel), dev_worktree=dev_worktree):
            problems.append(str(rel))
    return "dev-worktree" if dev_worktree else "directory", problems


def scan_zip(root: Path) -> tuple[str, list[str]]:
    problems: list[str] = []
    with zipfile.ZipFile(root) as zf:
        for member in zf.namelist():
            if forbidden(member):
                problems.append(member)
    return "zip", problems


def main() -> int:
    args = sys.argv[1:]
    dev_worktree = False
    if "--dev-worktree" in args:
        dev_worktree = True
        args.remove("--dev-worktree")
    root = Path(args[0]) if args else Path.cwd()
    if root.is_file() and root.suffix == ".zip":
        if dev_worktree:
            print("--dev-worktree is only valid for directory scans", file=sys.stderr)
            return 2
        mode, problems = scan_zip(root)
    else:
        mode, problems = scan_directory(root, dev_worktree=dev_worktree)
    if problems:
        print(f"release hygiene failed mode={mode}:")
        for item in sorted(set(problems)):
            print(f" - {item}")
        return 1
    print(f"release hygiene passed mode={mode}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
