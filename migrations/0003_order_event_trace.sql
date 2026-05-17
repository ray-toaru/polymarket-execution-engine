ALTER TABLE order_events
    ADD COLUMN IF NOT EXISTS correlation_id TEXT;

CREATE INDEX IF NOT EXISTS idx_order_events_order_correlation
    ON order_events(order_id, correlation_id, event_id DESC)
    WHERE correlation_id IS NOT NULL;
