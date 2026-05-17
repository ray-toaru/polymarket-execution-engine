# PostgreSQL-backed API E2E

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

v0.13 introduces an HTTP API constructor backed by `PostgresStore`:

```rust
pmx_api::try_postgres_app(database_url, apply_schema).await
```

The route graph is identical to the default in-memory API, but storage is backed by PostgreSQL through `pmx-service` and `pmx-store`.

## Test

`crates/pmx-api/tests/http_postgres_e2e.rs` verifies:

1. Auth config is required and service/admin tokens are used.
2. `/v1/health` reports `checks.database = postgres`.
3. normalize -> snapshot -> decision -> compile -> submit works through HTTP.
4. submit stores a blocked receipt in PostgreSQL.
5. replaying the same `execution_id + plan_hash + idempotency_key` returns the stored receipt with `200 OK`.
6. `GET /v1/submissions/{execution_id}` loads the PostgreSQL-persisted receipt.

## Evidence command

```bash
PMX_TEST_DATABASE_URL=postgres://pmx@127.0.0.1:55431/pmx \
  cargo test -p pmx-api --test http_postgres_e2e -- --nocapture --test-threads=1
```

`validation/run_current_gates.sh` runs this after migration and `pmx-store` repository proof when `PMX_TEST_DATABASE_URL` is set.

## Safety boundary

The PostgreSQL-backed E2E still returns `BLOCKED` submit receipts. It does not sign orders, call `post_order`, cancel live orders, or mutate Polymarket remote state.
