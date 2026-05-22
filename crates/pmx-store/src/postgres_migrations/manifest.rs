pub struct SchemaMigration {
    pub version: &'static str,
    pub sql: &'static str,
}

pub const SCHEMA_MIGRATIONS: &[SchemaMigration] = &[
    SchemaMigration {
        version: "0001_initial",
        sql: include_str!("../../../../migrations/0001_initial.sql"),
    },
    SchemaMigration {
        version: "0002_migration_framework",
        sql: include_str!("../../../../migrations/0002_migration_framework.sql"),
    },
    SchemaMigration {
        version: "0003_order_event_trace",
        sql: include_str!("../../../../migrations/0003_order_event_trace.sql"),
    },
    SchemaMigration {
        version: "0004_real_funds_canary",
        sql: include_str!("../../../../migrations/0004_real_funds_canary.sql"),
    },
    SchemaMigration {
        version: "0005_constraint_decision_snapshot_nullable",
        sql: include_str!("../../../../migrations/0005_constraint_decision_snapshot_nullable.sql"),
    },
    SchemaMigration {
        version: "0006_runtime_kill_switch_scope",
        sql: include_str!("../../../../migrations/0006_runtime_kill_switch_scope.sql"),
    },
    SchemaMigration {
        version: "0007_runtime_global_kill_switch",
        sql: include_str!("../../../../migrations/0007_runtime_global_kill_switch.sql"),
    },
    SchemaMigration {
        version: "0008_idempotency_owner_lease",
        sql: include_str!("../../../../migrations/0008_idempotency_owner_lease.sql"),
    },
];
