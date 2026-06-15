use super::*;

pub(crate) fn map_db_error(err: tokio_postgres::Error) -> StoreError {
    if let Some(db_error) = err.as_db_error() {
        if db_error.code() == &tokio_postgres::error::SqlState::UNIQUE_VIOLATION {
            return StoreError::Conflict(db_error.message().to_string());
        }
        if db_error.code() == &tokio_postgres::error::SqlState::FOREIGN_KEY_VIOLATION {
            return StoreError::NotFound(db_error.message().to_string());
        }
        if db_error.code() == &tokio_postgres::error::SqlState::CHECK_VIOLATION {
            return StoreError::Conflict(db_error.message().to_string());
        }
        if db_error.code() == &tokio_postgres::error::SqlState::T_R_SERIALIZATION_FAILURE {
            return StoreError::SerializationFailure;
        }
        return StoreError::DatabaseUnavailable(format!(
            "postgres sqlstate={} message={}",
            db_error.code().code(),
            db_error.message()
        ));
    }
    StoreError::DatabaseUnavailable(err.to_string())
}
