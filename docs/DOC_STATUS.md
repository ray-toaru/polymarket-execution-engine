# Execution-engine document status

Status: current v0.28.0 production-live-candidate documentation. The source
line remains non-live by default; live submit, live cancel, production
deployment, and repeat real-funds canary execution require a later reviewed
decision.

`../AGENTS.md` contains execution-engine-specific agent rules. Current
documents in this directory describe the v0.28.0 production-live-candidate
state. Historical continuation, review, and gate-confirmation notes live under
`docs/archive/` and are excluded from normal release packaging.

Current validation entrypoint:

```bash
./validation/run_current_gates.sh
```

`validation/run_current_gates_impl.sh` is the implementation used by the wrapper. Older `run_v0_x_gates.sh` files are archived and are not active release gates.

Current canary documents:

- `REAL_FUNDS_CANARY.md`: guarded real-funds preflight and live-submit preconditions.
- `REAL_FUNDS_CANARY_LIFECYCLE.md`: local run persistence, idempotency, remote-unknown freeze, and simulated reconcile behavior with no remote side effects.
- `REAL_FUNDS_CANARY_CLOSEOUT.md`: completed controlled-canary closeout semantics, evidence files, and limitations.
- `REAL_FUNDS_CANARY_SEMANTICS_AUDIT.md`: current trading-semantics evidence, corrections, attack review, and residual limits.

Current production-design documents:

- `PRODUCTION_LIVE_GATEWAY_SECURITY_DESIGN.md`: future real gateway,
  production submit/cancel, and generic live-read safety design. It is design
  input only and does not enable live wiring.
