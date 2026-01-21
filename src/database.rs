use std::env;
use std::sync::LazyLock;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

use crate::Task;

pub static DB: LazyLock<SqlitePool> = LazyLock::new(|| {
    let db_connection_str =
        env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://db/Todos.db".to_string());

    SqlitePoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(3))
        .connect_lazy(&db_connection_str)
        .expect("can't connect to database")
});

pub async fn get_tasks() -> anyhow::Result<Vec<Task>> {
    let pool = &*DB;

    let tasks = sqlx::query_as::<_, Task>("SELECT id, description, done FROM todos")
        .fetch_all(pool)
        .await?;
    Ok(tasks)
}

pub async fn get_task(id: i64) -> anyhow::Result<Task> {
    let pool = &*DB;

    let task = sqlx::query_as::<_, Task>("SELECT id, description, done FROM todos WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(task)
}

pub async fn create_task(desc: String) -> anyhow::Result<Task> {
    let pool = &*DB;

    let id = sqlx::query("INSERT INTO todos (description, done) VALUES (?, ?)")
        .bind(desc)
        .bind(false)
        .execute(pool)
        .await?
        .last_insert_rowid();
    get_task(id).await
}

pub async fn update_task_done(id: i64, done: bool) -> anyhow::Result<Task> {
    let pool = &*DB;

    sqlx::query("UPDATE todos SET done = ? WHERE id = ?")
        .bind(done)
        .bind(id)
        .execute(pool)
        .await?;
    get_task(id).await
}

pub async fn delete_task(id: i64) -> anyhow::Result<i64> {
    let pool = &*DB;

    sqlx::query("DELETE FROM todos WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(id)
}
