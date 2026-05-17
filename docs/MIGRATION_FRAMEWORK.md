# PostgreSQL migration framework

Status: v0.24 foundation, non-live.

## Goal

PostgreSQL schema changes after `0001_initial.sql` must be forward-only,
versioned, checksum-bound, and visible in validation evidence. New tables,
columns, indexes, and constraints should not be appended to the historical
initial migration.

## Current behavior

- `migrations/0001_initial.sql` remains the idempotent baseline for fresh and
  reused local validation databases.
- `migrations/0002_migration_framework.sql` creates `schema_migrations`.
- `PostgresStore::apply_schema()` applies the ordered embedded migration list
  and records each migration version with its SHA-256 checksum.
- If a recorded migration version has a different checksum, schema application
  fails closed with a conflict error.

## Validation

- PostgreSQL repository tests assert that `schema_migrations` records
  `0001_initial` and `0002_migration_framework`.
- `validation/check_migration_framework.py` guards the migration list, checksum
  failure path, PG test coverage, and evidence-manifest wiring.
- `validation/run_current_gates.sh` writes
  `evidence/current/logs/33-migration-framework-guard.log`.
- `validation/write_v0_23_evidence_manifest.py` records the
  `migration_framework_validation` evidence section.

## Boundary

This is not a production migration runner. It is the minimum durable framework
needed for v0.24 schema evolution and local promotion evidence. Backward
compatibility, dry-run, drift checks, and upgraded-DB/fresh-DB split evidence are
still required before any production-readiness claim.

