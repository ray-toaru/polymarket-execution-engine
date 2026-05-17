# Cancel / Reconcile State-machine Next Work

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

v0.22 keeps live cancel disabled and adds clearer reconcile classification in `pmx-core`.

Current core helper:

```text
RemoteUnknown -> QueryRemoteOpenOrder
PartialRemoteUnknown -> ConfirmMissingOrEscalate
Failed -> OperatorRequired
Other states -> Noop
```

Next non-live work:

- Add fake-gateway cancel lifecycle tests.
- Persist cancel lifecycle events in PostgreSQL.
- Model `not_canceled` as non-terminal unless reconcile confirms remote truth.
- Add stale `RemoteUnknown` escalation into operator-required reconcile.
- Ensure cancel/reconcile never claim terminal state based only on request submission.
