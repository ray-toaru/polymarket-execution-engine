ALTER TABLE real_funds_canary_runs
    DROP CONSTRAINT IF EXISTS real_funds_canary_runs_execution_style_check;

ALTER TABLE real_funds_canary_runs
    DROP CONSTRAINT IF EXISTS real_funds_canary_execution_style_check;

ALTER TABLE real_funds_canary_runs
    ADD CONSTRAINT real_funds_canary_execution_style_check
    CHECK (execution_style IN ('FOK_LIMIT_FILL', 'GTC_LIMIT_POST_ONLY_CANCEL'));
