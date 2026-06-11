# Contributing

Target `main` with a focused pull request. Follow the nearest `AGENTS.md` and
run the relevant Rust, PostgreSQL, SDK, validation, and documentation gates.

Live submit, live cancel, production deployment, and real-funds authorization
remain blocked. Never commit private keys, CLOB credentials, raw signatures,
signed payloads, signed order envelopes, or production database contents.

Use squash merge. Changes to credentialed workflows, signing boundaries,
authorization, migrations, release decisions, or live guards require an
independent external review reference until a second reviewer is available.

Release tags must be annotated and cryptographically signed. Existing unsigned
tags remain immutable historical records.
