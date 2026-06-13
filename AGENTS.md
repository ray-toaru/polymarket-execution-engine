# AGENTS.md — Polymarket execution engine

## Scope

Applies to this repository, the Rust execution plane. module-level `AGENTS.md` files under `crates/`, `adapters/`, `openapi/`, `migrations/`, and `validation/` add stricter local rules for high-risk boundaries.

## Boundary

- This is the only plane allowed to model signing-boundary isolation, lifecycle persistence, runtime state, authorization, and SDK integration scaffolding.
- Live submit, live cancel, and production deployment remain blocked unless a formally reviewed release decision changes that after full gates pass.
- SDK, sign-only, authenticated smoke, and dry-run paths must remain feature/env gated and no-remote-side-effect by default.
- Never expose private keys, CLOB secrets, raw signed payloads, raw signatures, or signed order envelopes through logs, OpenAPI responses, audit queries, or lifecycle events.

## Rust development rules

- Workspace MSRV: Rust `1.96`, edition `2024`.
- Keep `Cargo.lock` and adapter lockfiles consistent after dependency changes.
- Prefer small, behavior-preserving changes unless the task explicitly requires design work.
- Do not weaken fail-closed behavior for auth, runtime degraded status, live-submit guard, or payload redaction.
- If OpenAPI changes, update Rust schema generation/handlers, Hermes models, tests, and static guards.
- Large-file refactors should be done only with Rust gates available; avoid behavior changes during module moves.

## Validation

Full external gate from this directory:

```bash
./validation/run_current_gates.sh
```

Low-resource static checks from this directory:

```bash
python validation/check_plan_storage.py
python validation/check_live_submit_guard.py
python validation/check_sign_only_lifecycle.py
python validation/check_runtime_worker_models.py
# Current version-specific lifecycle and evidence guards are invoked by run_current_gates.sh.
```

Cross-repository contract and release-package checks, including lifecycle API parity and Hermes model parity, run from the integration repository.

Rust checks, when tooling is available:

```bash
cargo fmt --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked -- --test-threads=1
```

Optional environment-gated checks require explicit variables such as `PMX_TEST_DATABASE_URL`, `PMX_RUN_AUTHENTICATED_NON_TRADING_SMOKE`, and `PMX_RUN_SIGN_ONLY_DRY_RUN`.

## Evidence and docs

- Current evidence must stay under `evidence/current/`.
- Historical evidence and historical docs belong in archive directories and must not be included in normal release packages.
- Keep docs describing status, next gates, and release decisions accurate when changing gate status or promotion claims.
- Do not put current version numbers or current release status in AGENTS.md; use release and validation documents for that.
