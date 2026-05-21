use super::*;
use pmx_core::{
    AccountId, DecimalString, ExecutionId, OrderReservation, QuantityBound, ReservationState,
    SubmitReceipt, SubmitStatus,
};

#[tokio::test]
async fn remote_unknown_is_persisted_conservatively() {
    let Some(store) = test_store().await else {
        return;
    };
    let account = unique("acct");
    let execution = unique("exec");
    seed_execution_plan(&store, &account, &execution).await;
    let receipt = SubmitReceipt {
        execution_id: execution.clone(),
        receipt_id: unique("receipt"),
        status: SubmitStatus::RemoteUnknown,
        executor_version: "test".into(),
        contract_version: "test".into(),
    };
    store
        .record_submit_receipt(&receipt)
        .await
        .expect("record remote unknown receipt");
    let client = store.client().await.expect("test postgres client");
    let status: String = client
        .query_one(
            "SELECT status FROM submit_receipts WHERE execution_id = $1",
            &[&execution],
        )
        .await
        .expect("query receipt")
        .get(0);
    assert_eq!(status, "REMOTE_UNKNOWN");
}

#[tokio::test]
async fn reservation_double_spend_is_prevented_concurrently() {
    let Some(store) = test_store().await else {
        return;
    };
    let account = unique("acct");
    let execution = unique("exec");
    seed_execution_plan(&store, &account, &execution).await;
    let reservation = OrderReservation {
        reservation_id: unique("reservation"),
        account_id: AccountId(account.clone()),
        execution_id: ExecutionId(execution.clone()),
        internal_order_id: None,
        quantity_bound: QuantityBound::WorstCaseQuoteNotional(DecimalString("10".into())),
        state: ReservationState::Active,
    };
    let a = store.clone();
    let b = store.clone();
    let r1 = reservation.clone();
    let r2 = reservation;
    let (left, right) = tokio::join!(
        async move { a.save_order_reservation(&r1).await },
        async move { b.save_order_reservation(&r2).await }
    );
    assert!(left.is_ok() || right.is_ok());
    let client = store.client().await.expect("test postgres client");
    let count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM order_reservations WHERE account_id = $1 AND execution_id = $2",
            &[&account, &execution],
        )
        .await
        .expect("query reservations")
        .get(0);
    assert_eq!(count, 1);
}
