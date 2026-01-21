use sqlx::FromRow;

#[derive(Default, FromRow, Clone, Debug)]
pub struct Task {
    pub id: i64,
    pub description: String,
    pub done: bool,
}
