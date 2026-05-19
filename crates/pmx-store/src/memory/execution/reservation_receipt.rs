use super::*;

pub fn save_order_reservation(
    store: &InMemoryStore,
    reservation: &OrderReservation,
) -> Result<(), StoreError> {
    store
        .inner
        .lock()
        .expect("in-memory store mutex poisoned")
        .reservations
        .insert(reservation.reservation_id.clone(), reservation.clone());
    Ok(())
}

pub fn record_submit_receipt(
    store: &InMemoryStore,
    receipt: &SubmitReceipt,
) -> Result<(), StoreError> {
    store
        .inner
        .lock()
        .expect("in-memory store mutex poisoned")
        .receipts
        .insert(receipt.execution_id.clone(), receipt.clone());
    Ok(())
}

pub fn load_submit_receipt(
    store: &InMemoryStore,
    execution_id: &str,
) -> Result<SubmitReceipt, StoreError> {
    store
        .inner
        .lock()
        .expect("in-memory store mutex poisoned")
        .receipts
        .get(execution_id)
        .cloned()
        .ok_or_else(|| StoreError::NotFound(format!("execution_id={execution_id}")))
}
