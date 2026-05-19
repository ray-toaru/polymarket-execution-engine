# Production Secret Custody Drill

This drill is v0.27 productionization evidence for local secret-custody
controls. It is not a secret-manager, KMS, or HSM implementation and does not
claim production readiness.

Required checks:

- sensitive_env_detected_as_boolean_only
- sensitive_env_values_absent_from_logs
- sensitive_env_values_absent_from_manifest
- env_file_absent_from_artifact
- artifact_contains_no_env_file
- package_excludes_env_file
- no_plaintext_private_keys_logged
- no_clob_secret_logged
- rotation_drill_required
- break_glass_review_required

Required behavior:

```text
secret_values_logged = false
artifact_contains_env_file = false
remote_side_effects = false
production_ready_claimed = false
```

Passing this drill means local logs, evidence, and package artifacts do not
expose configured sensitive values observed by the validation process. It does
not replace external secret custody, credential rotation, break-glass review, or
hardware-backed signing.
