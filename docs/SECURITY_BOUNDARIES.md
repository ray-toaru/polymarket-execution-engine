# Security Boundaries

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

## Must Stay Inside Execution Engine

- private key material
- signer provider internals
- CLOB API secret
- HMAC preimage
- raw signed order payload
- reusable signed order envelope
- direct DB writes

## May Cross to Control Plane

- normalized intent
- feasibility snapshot
- constraint decision
- execution plan summary
- submit receipt
- cancel receipt
- health/report data

## Scope Model

- Service scope: normal control-plane operations.
- Admin scope: cancel, reconcile, kill switch and other operator actions.

Server-side authorization is mandatory. Client-side conventions are not sufficient.
