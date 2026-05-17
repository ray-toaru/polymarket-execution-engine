# Execution Engine Attack-Defense Notes

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

## Round 1

Conclusion: Rust executor owns all funds-bearing side effects.

Defense: this keeps signing, posting, cancellation and ledger in one service boundary.

Attack: Rust rewrite can introduce new bugs.

Valid: yes.

Revision: do not add real Polymarket adapter before core compile, fake gateway E2E and PostgreSQL tests pass.

## Round 2

Conclusion: API should expose plan summary, not signed order objects.

Defense: signed objects are reusable side-effect material and should not cross into control planes.

Attack: control plane may need detailed audit visibility.

Valid: partially.

Revision: expose structured explanations and audit reports, not raw signatures.

## Round 3

Conclusion: PostgreSQL should be the only production truth source.

Defense: dual SQLite/PostgreSQL truth increases drift risk.

Attack: SQLite helps local development.

Valid: partially.

Revision: use fake in-memory/test stores for local development, not a second production schema.
