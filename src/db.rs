use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};



pub async fn init_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
    .max_connections(5)
    .connect("sqlite:data/chat.db?mode=rwc") // Added data/ prefix
    .await
    .expect("Failed to create pool.");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            pfp_path TEXT
        );
        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            content TEXT NOT NULL,
            attachment_path TEXT,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(user_id) REFERENCES users(id)
        );"
    )
    .execute(&pool)
    .await
    .expect("Failed to initialize database schema");

    pool
}