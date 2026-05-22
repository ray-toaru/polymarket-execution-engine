CREATE TABLE IF NOT EXISTS runtime_global_controls (
    control_key TEXT PRIMARY KEY CHECK (control_key = 'kill_switch'),
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    control_version BIGINT NOT NULL DEFAULT 0,
    reason TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
