# Low-resource Rust Validation Notes

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

The official SDK `sdk-typecheck` path pulls a large dependency graph including Alloy, rustls/reqwest, ICU-related crates, and optional WebSocket/heartbeat dependencies. In constrained containers this can fail before business tests run.

Observed environment issue:

- cgroup `pids.max` was 2048;
- Rust/LLVM worker thread creation failed with `Resource temporarily unavailable`;
- the issue was stabilized by limiting parallelism and reducing debug info.

Recommended validation defaults:

```bash
export CARGO_BUILD_JOBS=1
export RUSTFLAGS='-C debuginfo=0'
```

If the system linker has problems, prefer a modern linker such as `lld`. The uploaded evidence used `gold` successfully but emitted a deprecation warning. Treat that warning as an environment note, not a business logic failure.

The v0.11 gate script applies conservative defaults unless the caller explicitly overrides them.
