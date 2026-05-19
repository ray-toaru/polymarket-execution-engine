use super::*;
use tokio_postgres::Client;

pub(crate) async fn load_json_payload<T: serde::de::DeserializeOwned>(
    client: &Client,
    table: &str,
    id_column: &str,
    id_value: &str,
    payload_column: &str,
) -> Result<T, StoreError> {
    let query = format!("SELECT {payload_column} FROM {table} WHERE {id_column} = $1");
    let row = client
        .query_opt(&query, &[&id_value])
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| StoreError::NotFound(format!("{table}.{id_column}={id_value}")))?;
    let payload: serde_json::Value = row.get(0);
    serde_json::from_value(payload).map_err(|err| StoreError::InvalidData(err.to_string()))
}
