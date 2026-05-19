CREATE TABLE IF NOT EXISTS real_funds_canary_runs (
    run_id TEXT PRIMARY KEY,
    execution_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    approval_hash TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    artifact_sha256 TEXT NOT NULL,
    evidence_manifest_sha256 TEXT NOT NULL,
    market_id TEXT NOT NULL,
    token_id_hash TEXT NOT NULL,
    max_order_notional_usd NUMERIC NOT NULL CHECK (max_order_notional_usd > 0),
    max_daily_notional_usd NUMERIC NOT NULL CHECK (max_daily_notional_usd > 0),
    order_notional_usd NUMERIC NOT NULL CHECK (order_notional_usd > 0),
    execution_style TEXT NOT NULL CHECK (execution_style = 'FOK_LIMIT_FILL'),
    remote_order_id TEXT,
    remote_status TEXT,
    lifecycle_state TEXT NOT NULL,
    remote_side_effects BOOLEAN NOT NULL DEFAULT FALSE,
    raw_signed_order_exposed BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (account_id, idempotency_key),
    CONSTRAINT real_funds_canary_approval_hash_len CHECK (length(approval_hash) = 64),
    CONSTRAINT real_funds_canary_artifact_hash_len CHECK (length(artifact_sha256) = 64),
    CONSTRAINT real_funds_canary_evidence_hash_len CHECK (length(evidence_manifest_sha256) = 64),
    CONSTRAINT real_funds_canary_token_hash_len CHECK (length(token_id_hash) = 64),
    CONSTRAINT real_funds_canary_no_raw_signed_order CHECK (raw_signed_order_exposed = FALSE)
);

CREATE INDEX IF NOT EXISTS idx_real_funds_canary_account_created
    ON real_funds_canary_runs (account_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_real_funds_canary_execution
    ON real_funds_canary_runs (execution_id);
