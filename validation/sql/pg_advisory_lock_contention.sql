-- Manual PostgreSQL advisory-lock contention probe.
-- Run this in two psql sessions with the same lock key to confirm same-resource serialization.
-- This is a primitive proof only; it does not prove full repository-level idempotency/reservation correctness.

-- Session A:
BEGIN;
SELECT clock_timestamp() AS a_lock_request_at;
SELECT pg_advisory_xact_lock(557);
SELECT clock_timestamp() AS a_lock_acquired_at;
SELECT pg_sleep(20);
COMMIT;
SELECT clock_timestamp() AS a_committed_at;

-- Session B, started while Session A is sleeping:
BEGIN;
SELECT clock_timestamp() AS b_lock_request_at;
SELECT pg_advisory_xact_lock(557);
SELECT clock_timestamp() AS b_lock_acquired_at;
COMMIT;
