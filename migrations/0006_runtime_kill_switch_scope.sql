ALTER TABLE IF EXISTS runtime_accounts
    ADD COLUMN IF NOT EXISTS kill_switch_version BIGINT NOT NULL DEFAULT 0;

ALTER TABLE IF EXISTS runtime_accounts
    ADD COLUMN IF NOT EXISTS kill_switch_reason TEXT;

ALTER TABLE IF EXISTS runtime_accounts
    ADD COLUMN IF NOT EXISTS kill_switch_updated_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_runtime_accounts_kill_switch
    ON runtime_accounts(kill_switch_enabled, kill_switch_updated_at DESC);
