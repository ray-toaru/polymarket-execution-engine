ALTER TABLE collateral_profiles
    ADD COLUMN IF NOT EXISTS account_id TEXT,
    ADD COLUMN IF NOT EXISTS condition_id TEXT;

CREATE INDEX IF NOT EXISTS idx_collateral_profiles_scope
    ON collateral_profiles(account_id, condition_id, status, created_at DESC);

ALTER TABLE worker_health
    ADD COLUMN IF NOT EXISTS account_id TEXT,
    ADD COLUMN IF NOT EXISTS condition_id TEXT,
    ADD COLUMN IF NOT EXISTS worker_group TEXT;

CREATE INDEX IF NOT EXISTS idx_worker_health_scope_capability
    ON worker_health(account_id, condition_id, capability, updated_at DESC);
