-- Align upgraded databases with the current baseline: decisions can be stored
-- without duplicating snapshot_id because the canonical decision payload is
-- keyed and validated through the object graph.
ALTER TABLE constraint_decisions
  ALTER COLUMN snapshot_id DROP NOT NULL;
