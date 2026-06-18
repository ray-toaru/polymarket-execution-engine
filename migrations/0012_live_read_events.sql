CREATE TABLE IF NOT EXISTS live_read_events (
    event_id BIGSERIAL PRIMARY KEY,
    account_id TEXT NOT NULL,
    operation TEXT NOT NULL,
    outcome TEXT NOT NULL,
    remote_order_id TEXT,
    remote_state TEXT,
    error_category TEXT,
    redacted_error_summary TEXT,
    no_trading_side_effect BOOLEAN NOT NULL DEFAULT TRUE,
    redacted_fields JSONB NOT NULL DEFAULT '[]'::jsonb,
    observed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (no_trading_side_effect = TRUE)
);

CREATE INDEX IF NOT EXISTS idx_live_read_events_account_event_id
    ON live_read_events(account_id, event_id DESC);
CREATE INDEX IF NOT EXISTS idx_live_read_events_operation_event_id
    ON live_read_events(operation, event_id DESC);
CREATE INDEX IF NOT EXISTS idx_live_read_events_outcome_event_id
    ON live_read_events(outcome, event_id DESC);
CREATE INDEX IF NOT EXISTS idx_live_read_events_remote_order_id
    ON live_read_events(remote_order_id, event_id DESC);
