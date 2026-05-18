use pmx_core::OrderReservation;

use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{
    StoreError, advisory_lock_key, quantity_bound_to_resource_and_amount, reservation_state_to_str,
};

pub(super) async fn save_order_reservation(
    store: &PostgresStore,
    reservation: &OrderReservation,
) -> Result<(), StoreError> {
    let (resource_kind, amount) =
        quantity_bound_to_resource_and_amount(&reservation.quantity_bound)?;
    let lock = advisory_lock_key(
        "reservation",
        &reservation.account_id.0,
        &format!("{}:{resource_kind}", reservation.execution_id.0),
    );
    let client = store.client().await?;
    client.batch_execute("BEGIN").await.map_err(map_db_error)?;
    if let Err(err) = client
        .execute("SELECT pg_advisory_xact_lock($1)", &[&lock.0])
        .await
    {
        PostgresStore::rollback(&client).await;
        return Err(map_db_error(err));
    }
    let order_id: Option<&str> = reservation.internal_order_id.as_ref().map(|v| v.0.as_str());
    let result = client
        .execute(
            "INSERT INTO order_reservations (reservation_id, order_id, execution_id, account_id, resource_kind, amount, state) \
             VALUES ($1, $2, $3, $4, $5, $6::text::numeric, $7) \
             ON CONFLICT (reservation_id) DO UPDATE SET state = EXCLUDED.state",
            &[
                &reservation.reservation_id,
                &order_id,
                &reservation.execution_id.0,
                &reservation.account_id.0,
                &resource_kind,
                &amount,
                &reservation_state_to_str(&reservation.state),
            ],
        )
        .await;
    match result {
        Ok(_) => {
            client.batch_execute("COMMIT").await.map_err(map_db_error)?;
            Ok(())
        }
        Err(err) => {
            PostgresStore::rollback(&client).await;
            Err(map_db_error(err))
        }
    }
}
