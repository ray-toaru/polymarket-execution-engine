from __future__ import annotations

from pathlib import Path


FORBIDDEN_PARTS = {
    ".git",
    ".venv",
    "venv",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    "target",
    "dist",
    "secrets",
}
FORBIDDEN_SUFFIXES = {
    ".pyc",
    ".pyo",
    ".db",
    ".sqlite",
    ".sqlite3",
    ".pem",
    ".key",
    ".crt",
    ".p12",
    ".pfx",
    ".asc",
    ".gpg",
    ".age",
    ".kdbx",
}
FORBIDDEN_FILENAMES = {".env", "config/secrets.json"}
FORBIDDEN_NAME_PREFIXES = (".env.",)
FORBIDDEN_NAME_SUFFIXES = (".local.json",)
EXCLUDED_PREFIXES = {
    "docs/archive",
    "external_reviews",
    "validation/archive",
    "polymarket-execution-engine/validation/archive",
    "polymarket-execution-engine/evidence/archive",
    "polymarket-execution-engine/docs/archive",
}
ALLOWED_ROOT_FILES = {
    ".env.example",
    ".github",
    ".gitignore",
    ".gitmodules",
    "AGENTS.md",
    "COMPONENT_COMPATIBILITY.md",
    "CONTROLLED_CANARY_CLOSEOUT.md",
    "CURRENT_PROGRESS.md",
    "DEPENDENCY_POLICY.md",
    "DESIGN_DECISION_RECORD.md",
    "DEVELOPMENT_HANDOFF.md",
    "DOC_STATUS.md",
    "IMPLEMENTATION_STATUS.md",
    "NO_LOCAL_ACTIONS_REMAINING.md",
    "PROJECT_ARCHITECTURE.md",
    "README.md",
    "RELEASE_DECISION.md",
    "REVIEW_AUDIT.md",
    "ROADMAP.md",
    "TASKS.md",
    "VALIDATION_REPORT.md",
    "VERSION",
    "constraints-ci.txt",
    "requirements-ci.txt",
}
ALLOWED_ROOT_DIRS = {
    ".github",
    "docs",
    "hermes-polymarket-executor-adapter",
    "polymarket-execution-engine",
    "scripts",
    "tests",
}


def is_allowed_release_source_path(path: str | Path, *, expected_root: str | None = None) -> bool:
    raw = path.as_posix() if isinstance(path, Path) else str(path)
    normalized = raw.rstrip("/")
    rel = normalized
    if expected_root:
        prefix = expected_root + "/"
        if rel.startswith(prefix):
            rel = rel[len(prefix) :]
        elif rel == expected_root:
            rel = ""
    if not rel:
        return False
    rel_path = Path(rel)
    parts = rel_path.parts
    if not parts:
        return False
    top = parts[0]
    if len(parts) == 1:
        return top in ALLOWED_ROOT_FILES or top in ALLOWED_ROOT_DIRS
    return top in ALLOWED_ROOT_DIRS


def is_forbidden_release_member(path: str | Path, *, expected_root: str | None = None) -> bool:
    raw = path.as_posix() if isinstance(path, Path) else str(path)
    normalized = raw.rstrip("/")
    rel = normalized
    if expected_root:
        prefix = expected_root + "/"
        if rel.startswith(prefix):
            rel = rel[len(prefix) :]
        elif rel == expected_root:
            rel = ""
    rel_path = Path(rel)
    parts = rel_path.parts
    name = rel_path.name if parts else rel

    if any(part in FORBIDDEN_PARTS for part in parts):
        return True
    if "logs" in parts:
        return True
    if any(part.endswith(".egg-info") for part in parts):
        return True
    if rel in FORBIDDEN_FILENAMES or name in FORBIDDEN_FILENAMES:
        return True
    if any(name.startswith(prefix) for prefix in FORBIDDEN_NAME_PREFIXES) and not name.endswith(".example"):
        return True
    if any(name.endswith(suffix) for suffix in FORBIDDEN_NAME_SUFFIXES):
        return True
    if rel_path.suffix in FORBIDDEN_SUFFIXES:
        return True
    if any(rel == prefix or rel.startswith(prefix + "/") for prefix in EXCLUDED_PREFIXES):
        return True
    return False
