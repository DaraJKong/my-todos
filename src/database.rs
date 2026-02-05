use std::env;
use std::sync::LazyLock;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

use crate::core::ServerError;
use crate::{Priority, Status, Task};

pub static DB: LazyLock<SqlitePool> = LazyLock::new(|| {
    let db_connection_str =
        env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://db/Todos.db".to_string());

    SqlitePoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(3))
        .connect_lazy(&db_connection_str)
        .expect("can't connect to database")
});

pub async fn get_tasks() -> Result<Vec<Task>, ServerError> {
    let pool = &*DB;

    #[cfg(debug_assertions)]
    std::thread::sleep(Duration::from_millis(500));

    let tasks = sqlx::query_as::<_, Task>("SELECT id, description, status, priority FROM todos")
        .fetch_all(pool)
        .await?;
    Ok(tasks)
}

pub async fn get_task(id: i64) -> Result<Task, ServerError> {
    let pool = &*DB;

    #[cfg(debug_assertions)]
    std::thread::sleep(Duration::from_millis(500));

    let task = sqlx::query_as::<_, Task>(
        "SELECT id, description, status, priority FROM todos WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(task)
}

pub async fn create_task(desc: String) -> Result<Task, ServerError> {
    let pool = &*DB;

    #[cfg(debug_assertions)]
    std::thread::sleep(Duration::from_millis(500));

    let id = sqlx::query("INSERT INTO todos (description) VALUES (?)")
        .bind(desc)
        .execute(pool)
        .await?
        .last_insert_rowid();
    get_task(id).await
}

pub async fn update_task(
    id: i64,
    desc: String,
    status: Status,
    priority: Priority,
) -> Result<Task, ServerError> {
    let pool = &*DB;

    #[cfg(debug_assertions)]
    std::thread::sleep(Duration::from_millis(500));

    sqlx::query("UPDATE todos SET description = ?, status = ?, priority = ? WHERE id = ?")
        .bind(desc)
        .bind(status)
        .bind(priority)
        .bind(id)
        .execute(pool)
        .await?;
    get_task(id).await
}

pub async fn delete_task(id: i64) -> Result<i64, ServerError> {
    let pool = &*DB;

    #[cfg(debug_assertions)]
    std::thread::sleep(Duration::from_millis(500));

    sqlx::query("DELETE FROM todos WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(id)
}
