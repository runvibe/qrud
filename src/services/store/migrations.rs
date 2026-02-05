use sqlx::{PgPool, SqlitePool};

pub(super) async fn migrate_sqlite(pool: &SqlitePool) -> Result<(), String> {
    sqlx::migrate!("./migrations/sqlite")
        .run(pool)
        .await
        .map_err(|err| err.to_string())
}

pub(super) async fn migrate_postgres(pool: &PgPool) -> Result<(), String> {
    sqlx::migrate!("./migrations/postgres")
        .run(pool)
        .await
        .map_err(|err| err.to_string())
}
