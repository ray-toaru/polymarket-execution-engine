-- PostgreSQL production truth source. SQLite is intentionally not supported.
-- v0.20 schema baseline.
-- execution_plans is the single canonical plan table; plan_summaries was removed before deployment to prevent dual-table drift.
DROP TABLE IF EXISTS plan_summaries;

CREATE TABLE IF NOT EXISTS runtime_accounts (
    account_id TEXT PRIMARY KEY,
    status TEXT NOT NULL,
    kill_switch_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS runtime_markets (
    condition_id TEXT PRIMARY KEY,
    status TEXT NOT NULL,
    is_sports BOOLEAN NOT NULL DEFAULT FALSE,
    last_market_event_at TIMESTAMPTZ,
    last_user_event_at TIMESTAMPTZ,
    last_sports_channel_event_at TIMESTAMPTZ,
    last_sports_subscription_event_at TIMESTAMPTZ,
    last_sports_entity_event_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS collateral_profiles (
    profile_id TEXT PRIMARY KEY,
    status TEXT NOT NULL,
    quote_asset_symbol TEXT NOT NULL,
    quote_asset_address TEXT NOT NULL,
    allowance_target TEXT NOT NULL,
    decimals SMALLINT NOT NULL CHECK (decimals >= 0),
    profile_version TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS worker_health (
    worker_id TEXT PRIMARY KEY,
    role TEXT NOT NULL,
    capability TEXT NOT NULL,
    status TEXT NOT NULL,
    last_heartbeat_at TIMESTAMPTZ NOT NULL,
    last_error TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS normalized_intents (
    normalized_intent_id TEXT PRIMARY KEY,
    intent_hash TEXT NOT NULL UNIQUE,
    account_id TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS feasibility_snapshots (
    snapshot_id TEXT PRIMARY KEY,
    snapshot_hash TEXT NOT NULL UNIQUE,
    normalized_intent_id TEXT NOT NULL REFERENCES normalized_intents(normalized_intent_id),
    payload JSONB NOT NULL,
    captured_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS constraint_decisions (
    decision_id TEXT PRIMARY KEY,
    decision_hash TEXT NOT NULL UNIQUE,
    snapshot_id TEXT REFERENCES feasibility_snapshots(snapshot_id),
    status TEXT NOT NULL,
    reasons JSONB NOT NULL DEFAULT '[]'::jsonb,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS execution_plans (
    execution_id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    normalized_intent_id TEXT NOT NULL REFERENCES normalized_intents(normalized_intent_id),
    snapshot_id TEXT REFERENCES feasibility_snapshots(snapshot_id),
    decision_id TEXT NOT NULL REFERENCES constraint_decisions(decision_id),
    plan_hash TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    summary_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS idempotency_records (
    idempotency_record_id BIGSERIAL PRIMARY KEY,
    account_id TEXT NOT NULL,
    execution_id TEXT NOT NULL REFERENCES execution_plans(execution_id),
    idempotency_key TEXT NOT NULL,
    submit_attempt INTEGER NOT NULL CHECK (submit_attempt >= 1),
    request_fingerprint TEXT NOT NULL,
    status TEXT NOT NULL,
    response_fingerprint TEXT,
    response_json JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(account_id, execution_id, idempotency_key),
    UNIQUE(account_id, execution_id, submit_attempt)
);

CREATE TABLE IF NOT EXISTS orders (
    order_id TEXT PRIMARY KEY,
    execution_id TEXT NOT NULL REFERENCES execution_plans(execution_id),
    account_id TEXT NOT NULL,
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    side TEXT NOT NULL,
    lifecycle_state TEXT NOT NULL,
    remote_order_id TEXT,
    remote_state TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS order_events (
    event_id BIGSERIAL PRIMARY KEY,
    order_id TEXT NOT NULL REFERENCES orders(order_id),
    event_type TEXT NOT NULL,
    event_source TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS order_reservations (
    reservation_id TEXT PRIMARY KEY,
    order_id TEXT REFERENCES orders(order_id),
    execution_id TEXT NOT NULL REFERENCES execution_plans(execution_id),
    account_id TEXT NOT NULL,
    resource_kind TEXT NOT NULL,
    amount NUMERIC NOT NULL CHECK (amount > 0),
    state TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    released_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS execution_lifecycle_events (
    event_id BIGSERIAL PRIMARY KEY,
    execution_id TEXT NOT NULL REFERENCES execution_plans(execution_id),
    account_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_source TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_execution_lifecycle_execution_created
    ON execution_lifecycle_events(execution_id, created_at);

CREATE INDEX IF NOT EXISTS idx_execution_lifecycle_execution_event_id
    ON execution_lifecycle_events(execution_id, event_id DESC);

CREATE TABLE IF NOT EXISTS sign_only_lifecycle_events (
    event_id BIGSERIAL PRIMARY KEY,
    execution_id TEXT NOT NULL REFERENCES execution_plans(execution_id),
    account_id TEXT NOT NULL,
    state TEXT NOT NULL,
    event_type TEXT NOT NULL,
    client_event_id TEXT,
    signed_order_ref TEXT,
    no_remote_side_effect BOOLEAN NOT NULL DEFAULT TRUE,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (no_remote_side_effect = TRUE)
);

CREATE TABLE IF NOT EXISTS runtime_worker_observations (
    observation_id BIGSERIAL PRIMARY KEY,
    account_id TEXT NOT NULL,
    capability TEXT NOT NULL,
    worker_kind TEXT NOT NULL,
    status TEXT NOT NULL,
    should_fail_closed BOOLEAN NOT NULL,
    reason TEXT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS reconcile_runs (
    reconcile_id TEXT PRIMARY KEY,
    account_id TEXT,
    order_id TEXT REFERENCES orders(order_id),
    reason TEXT NOT NULL,
    status TEXT NOT NULL,
    result_json JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);

-- plan_summaries intentionally removed. Use execution_plans.summary_json as canonical plan summary storage.

CREATE TABLE IF NOT EXISTS submit_receipts (
    execution_id TEXT PRIMARY KEY,
    receipt_id TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    executor_version TEXT NOT NULL,
    contract_version TEXT NOT NULL,
    response_json JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);


CREATE TABLE IF NOT EXISTS admin_audit_events (
    audit_id BIGSERIAL PRIMARY KEY,
    principal_subject TEXT NOT NULL,
    operation TEXT NOT NULL,
    request_fingerprint TEXT,
    correlation_id TEXT,
    result TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);


-- v0.23 source-candidate forward-compatible column additions for reused test databases.
-- CREATE TABLE IF NOT EXISTS does not evolve existing tables, so keep these ALTERs idempotent.
ALTER TABLE IF EXISTS sign_only_lifecycle_events
    ADD COLUMN IF NOT EXISTS client_event_id TEXT;
ALTER TABLE IF EXISTS sign_only_lifecycle_events
    ADD COLUMN IF NOT EXISTS signed_order_ref TEXT;
ALTER TABLE IF EXISTS sign_only_lifecycle_events
    ADD COLUMN IF NOT EXISTS no_remote_side_effect BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE IF EXISTS sign_only_lifecycle_events
    ADD COLUMN IF NOT EXISTS payload JSONB NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE IF EXISTS runtime_worker_observations
    ADD COLUMN IF NOT EXISTS observed_at TIMESTAMPTZ NOT NULL DEFAULT now();
ALTER TABLE IF EXISTS admin_audit_events
    ADD COLUMN IF NOT EXISTS correlation_id TEXT;

CREATE INDEX IF NOT EXISTS idx_sign_only_lifecycle_execution_created
    ON sign_only_lifecycle_events(execution_id, created_at);
CREATE INDEX IF NOT EXISTS idx_sign_only_lifecycle_execution_event_id
    ON sign_only_lifecycle_events(execution_id, event_id DESC);
CREATE UNIQUE INDEX IF NOT EXISTS uq_sign_only_lifecycle_client_event
    ON sign_only_lifecycle_events(execution_id, client_event_id)
    WHERE client_event_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_runtime_worker_observations_account_created
    ON runtime_worker_observations(account_id, observed_at);
CREATE INDEX IF NOT EXISTS idx_runtime_worker_observations_account_capability_observed
    ON runtime_worker_observations(account_id, capability, observed_at DESC);
CREATE INDEX IF NOT EXISTS idx_orders_account_state ON orders(account_id, lifecycle_state);
CREATE INDEX IF NOT EXISTS idx_order_events_order_created ON order_events(order_id, created_at);
CREATE INDEX IF NOT EXISTS idx_reservations_account_state ON order_reservations(account_id, state);
CREATE UNIQUE INDEX IF NOT EXISTS uq_active_reservation_execution_resource
    ON order_reservations(account_id, execution_id, resource_kind)
    WHERE state IN ('PENDING', 'ACTIVE');
CREATE INDEX IF NOT EXISTS idx_worker_health_capability ON worker_health(capability, status);
CREATE INDEX IF NOT EXISTS idx_decisions_snapshot ON constraint_decisions(snapshot_id);
CREATE INDEX IF NOT EXISTS idx_idempotency_execution ON idempotency_records(account_id, execution_id);
CREATE INDEX IF NOT EXISTS idx_admin_audit_operation ON admin_audit_events(operation, created_at);
CREATE INDEX IF NOT EXISTS idx_admin_audit_operation_audit_id
    ON admin_audit_events(operation, audit_id DESC);

CREATE INDEX IF NOT EXISTS idx_admin_audit_principal_audit_id
    ON admin_audit_events(principal_subject, audit_id DESC);
CREATE INDEX IF NOT EXISTS idx_admin_audit_result_audit_id
    ON admin_audit_events(result, audit_id DESC);
CREATE INDEX IF NOT EXISTS idx_admin_audit_correlation_id
    ON admin_audit_events(correlation_id, audit_id DESC);
