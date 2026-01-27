use sqlx::{Error as SqlxError, FromRow};
use thiserror::Error;

#[derive(Default, FromRow, Clone, Debug)]
pub struct Task {
    pub id: i64,
    pub description: String,
    pub done: bool,
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("received a database error")]
    Database(SqlxError),
}

impl From<SqlxError> for ServerError {
    fn from(value: SqlxError) -> Self {
        Self::Database(value)
    }
}
