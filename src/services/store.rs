use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Sqlite, SqlitePool};

#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    pub async fn open(path: &str) -> Result<Self, String> {
        let options = if path == ":memory:" {
            SqliteConnectOptions::new()
                .in_memory(true)
                .shared_cache(true)
        } else {
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
        };

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|err| err.to_string())?;
        init_db(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn next_id_for(&self, collection: &str) -> Result<i64, String> {
        let mut tx = self.pool.begin().await.map_err(|err| err.to_string())?;

        let next_id = match sqlx::query_scalar::<_, i64>(
            "SELECT next_id FROM counters WHERE collection = ?;",
        )
        .bind(collection)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|err| err.to_string())?
        {
            Some(value) => value,
            None => max_id_for(&mut tx, collection).await? + 1,
        };

        sqlx::query(
            "INSERT INTO counters (collection, next_id)
             VALUES (?, ?)
             ON CONFLICT(collection) DO UPDATE SET next_id = excluded.next_id;",
        )
        .bind(collection)
        .bind(next_id + 1)
        .execute(tx.as_mut())
        .await
        .map_err(|err| err.to_string())?;

        tx.commit().await.map_err(|err| err.to_string())?;
        Ok(next_id)
    }

    pub async fn bump_next_id(&self, collection: &str, used_id: i64) -> Result<(), String> {
        let mut tx = self.pool.begin().await.map_err(|err| err.to_string())?;

        let current_next = match sqlx::query_scalar::<_, i64>(
            "SELECT next_id FROM counters WHERE collection = ?;",
        )
        .bind(collection)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|err| err.to_string())?
        {
            Some(value) => value,
            None => max_id_for(&mut tx, collection).await? + 1,
        };

        let desired_next = (used_id + 1).max(current_next);
        sqlx::query(
            "INSERT INTO counters (collection, next_id)
             VALUES (?, ?)
             ON CONFLICT(collection) DO UPDATE SET next_id = excluded.next_id;",
        )
        .bind(collection)
        .bind(desired_next)
        .execute(tx.as_mut())
        .await
        .map_err(|err| err.to_string())?;

        tx.commit().await.map_err(|err| err.to_string())?;
        Ok(())
    }

    pub async fn insert_item(
        &self,
        collection: &str,
        id: i64,
        data: &str,
    ) -> Result<(), String> {
        sqlx::query("INSERT INTO items (collection, id, data) VALUES (?, ?, ?);")
            .bind(collection)
            .bind(id)
            .bind(data)
            .execute(&self.pool)
            .await
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub async fn upsert_item(
        &self,
        collection: &str,
        id: i64,
        data: &str,
    ) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO items (collection, id, data)
             VALUES (?, ?, ?)
             ON CONFLICT(collection, id) DO UPDATE SET data = excluded.data;",
        )
        .bind(collection)
        .bind(id)
        .bind(data)
        .execute(&self.pool)
        .await
        .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub async fn update_item(
        &self,
        collection: &str,
        id: i64,
        data: &str,
    ) -> Result<(), String> {
        sqlx::query("UPDATE items SET data = ? WHERE collection = ? AND id = ?;")
            .bind(data)
            .bind(collection)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub async fn item_exists(&self, collection: &str, id: i64) -> Result<bool, String> {
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT 1 FROM items WHERE collection = ? AND id = ? LIMIT 1;",
        )
        .bind(collection)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| err.to_string())?
        .is_some();
        Ok(exists)
    }

    pub async fn fetch_item_data(
        &self,
        collection: &str,
        id: i64,
    ) -> Result<Option<String>, String> {
        sqlx::query_scalar::<_, String>(
            "SELECT data FROM items WHERE collection = ? AND id = ?;",
        )
        .bind(collection)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| err.to_string())
    }

    pub async fn delete_item(&self, collection: &str, id: i64) -> Result<bool, String> {
        let result = sqlx::query("DELETE FROM items WHERE collection = ? AND id = ?;")
            .bind(collection)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|err| err.to_string())?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_collection(&self, collection: &str) -> Result<Vec<String>, String> {
        sqlx::query_scalar::<_, String>(
            "SELECT data FROM items WHERE collection = ? ORDER BY id ASC;",
        )
        .bind(collection)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| err.to_string())
    }
}

async fn init_db(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS items (
            collection TEXT NOT NULL,
            id INTEGER NOT NULL,
            data TEXT NOT NULL,
            PRIMARY KEY (collection, id)
        );",
    )
    .execute(pool)
    .await
    .map_err(|err| err.to_string())?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS counters (
            collection TEXT PRIMARY KEY,
            next_id INTEGER NOT NULL
        );",
    )
    .execute(pool)
    .await
    .map_err(|err| err.to_string())?;
    Ok(())
}

async fn max_id_for(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    collection: &str,
) -> Result<i64, String> {
    sqlx::query_scalar::<_, i64>("SELECT COALESCE(MAX(id), 0) FROM items WHERE collection = ?;")
        .bind(collection)
        .fetch_one(tx.as_mut())
        .await
        .map_err(|err| err.to_string())
}
