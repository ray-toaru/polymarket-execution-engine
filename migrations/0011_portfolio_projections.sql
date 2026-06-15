CREATE TABLE IF NOT EXISTS portfolio_projections (
    account_id TEXT PRIMARY KEY,
    projection_json JSONB NOT NULL,
    observed_at_ms BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_portfolio_projections_observed_at
    ON portfolio_projections(observed_at_ms DESC);
