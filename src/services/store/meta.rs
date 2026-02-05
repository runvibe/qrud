use super::util::now_millis;
use super::{Backend, Store};

impl Store {
    pub async fn fetch_meta_pk_total(
        &self,
        workspace_id: &str,
        pk: &str,
    ) -> Result<Option<i64>, String> {
        match &self.backend {
            Backend::Sqlite(pool) => {
                let total = sqlx::query_scalar::<_, i64>(
                    "SELECT total FROM meta_pk WHERE workspace_id = ? AND pk = ?;",
                )
                .bind(workspace_id)
                .bind(pk)
                .fetch_optional(pool)
                .await
                .map_err(|err| err.to_string())?;
                Ok(total)
            }
            Backend::Postgres(pool) => {
                let total = sqlx::query_scalar::<_, i64>(
                    "SELECT total FROM meta_pk WHERE workspace_id = $1 AND pk = $2;",
                )
                .bind(workspace_id)
                .bind(pk)
                .fetch_optional(pool)
                .await
                .map_err(|err| err.to_string())?;
                Ok(total)
            }
        }
    }

    pub(super) async fn increment_meta_pk(
        &self,
        workspace_id: &str,
        pk: &str,
    ) -> Result<(), String> {
        let now = now_millis();
        match &self.backend {
            Backend::Sqlite(pool) => {
                sqlx::query(
                    "INSERT INTO meta_pk (workspace_id, pk, total, created_at, updated_at)
                     VALUES (?, ?, 1, ?, ?)
                     ON CONFLICT(workspace_id, pk)
                     DO UPDATE SET total = total + 1, updated_at = excluded.updated_at;",
                )
                .bind(workspace_id)
                .bind(pk)
                .bind(now)
                .bind(now)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?;
            }
            Backend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO meta_pk (workspace_id, pk, total, created_at, updated_at)
                     VALUES ($1, $2, 1, $3, $4)
                     ON CONFLICT (workspace_id, pk)
                     DO UPDATE SET total = meta_pk.total + 1, updated_at = EXCLUDED.updated_at;",
                )
                .bind(workspace_id)
                .bind(pk)
                .bind(now)
                .bind(now)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?;
            }
        }
        Ok(())
    }

    pub(super) async fn decrement_meta_pk(
        &self,
        workspace_id: &str,
        pk: &str,
    ) -> Result<(), String> {
        let now = now_millis();
        match &self.backend {
            Backend::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE meta_pk
                     SET total = CASE WHEN total > 0 THEN total - 1 ELSE 0 END,
                         updated_at = ?
                     WHERE workspace_id = ? AND pk = ?;",
                )
                .bind(now)
                .bind(workspace_id)
                .bind(pk)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?;
            }
            Backend::Postgres(pool) => {
                sqlx::query(
                    "UPDATE meta_pk
                     SET total = CASE WHEN total > 0 THEN total - 1 ELSE 0 END,
                         updated_at = $1
                     WHERE workspace_id = $2 AND pk = $3;",
                )
                .bind(now)
                .bind(workspace_id)
                .bind(pk)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?;
            }
        }
        Ok(())
    }
}
