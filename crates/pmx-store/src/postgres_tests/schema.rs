use super::*;

#[tokio::test]
async fn postgres_records_schema_migrations() {
    let Some(store) = test_store().await else {
        return;
    };
    let migrations = store
        .applied_schema_migrations()
        .await
        .expect("schema migration rows");
    assert!(
        migrations
            .iter()
            .any(|(version, checksum)| version == "0001_initial" && checksum.len() == 64)
    );
    assert!(
        migrations.iter().any(
            |(version, checksum)| version == "0002_migration_framework" && checksum.len() == 64
        )
    );
    assert!(
        migrations
            .iter()
            .any(|(version, checksum)| version == "0003_order_event_trace" && checksum.len() == 64)
    );
    assert!(
        migrations
            .iter()
            .any(|(version, checksum)| version == "0004_real_funds_canary" && checksum.len() == 64)
    );
    assert!(migrations.iter().any(|(version, checksum)| {
        version == "0005_constraint_decision_snapshot_nullable" && checksum.len() == 64
    }));
    assert!(migrations.iter().any(|(version, checksum)| {
        version == "0011_portfolio_projections" && checksum.len() == 64
    }));
    assert!(
        migrations
            .iter()
            .any(|(version, checksum)| version == "0012_live_read_events" && checksum.len() == 64)
    );
}
