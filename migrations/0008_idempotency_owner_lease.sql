ALTER TABLE idempotency_records
    ADD COLUMN IF NOT EXISTS owner_token TEXT,
    ADD COLUMN IF NOT EXISTS lease_expires_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_idempotency_owner_lease
    ON idempotency_records(account_id, execution_id, idempotency_key, owner_token, lease_expires_at);
