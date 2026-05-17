# SDK Regression Suite

The official SDK adapter regression suite is the v0.25 standard sign-only
baseline. It must stay no remote trading side effect by default: no order post,
no live cancel, no raw signed payload exposure, and no private key or CLOB
secret exposure.

Required coverage:

- mapping snapshot for limit, market, GTC, IOC-to-FAK, builder attribution, fee
  metadata, funder, signer, and signature type;
- redaction for named secrets, private-key-like values, normalized SDK errors,
  and gateway error conversion;
- error normalization for validation failures, status-code remote unknowns, and
  retryable WebSocket or remote-unknown paths;
- geoblock conversion from SDK status to core runtime status;
- read-only authenticated smoke boundaries: ambient credentials must not make
  the read-only path authenticated;
- sign-only dry-run boundaries: dry-run requires explicit opt-in and must not be
  accepted when live submit is enabled.

Validation entrypoint:

```bash
python validation/check_sdk_regression_suite.py
```

The full evidence gate records this as
`37-sdk-regression-suite-guard.log`. Passing the guard proves regression coverage
and static no-post/no-cancel checks for this source tree; it is not evidence of
live submit, live cancel, or production readiness.
