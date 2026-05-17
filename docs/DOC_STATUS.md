# Execution-engine document status

> Status: current v0.23.0 source-candidate documentation.

`../AGENTS.md` contains execution-engine-specific agent rules. Current documents in this directory describe the v0.23.0 source-candidate state. Historical continuation, review, and gate-confirmation notes live under `docs/archive/` and are excluded from normal release packaging.

Current validation entrypoint:

```bash
./validation/run_current_gates.sh
```

`validation/run_v0_23_gates.sh` is the version-pinned implementation used by the wrapper. Older `run_v0_x_gates.sh` files are archived and are not active release gates.
