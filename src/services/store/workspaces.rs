use crate::models::Workspace;
use crate::services::DEFAULT_WORKSPACE_NAME;

use super::mappers::{workspace_from_postgres, workspace_from_sqlite};
use super::util::now_millis;
use super::{Backend, Store};

impl Store {
    pub async fn create_workspace(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<Workspace, String> {
        if self.workspace_exists_by_name(name).await? {
            return Err("Workspace already exists".to_string());
        }
        let id = super::util::new_uuid();
        let now = now_millis();
        match &self.backend {
            Backend::Sqlite(pool) => {
                sqlx::query(
                    "INSERT INTO workspaces (id, name, description, created_at, updated_at, deleted_at)
                     VALUES (?, ?, ?, ?, ?, NULL);",
                )
                .bind(&id)
                .bind(name)
                .bind(description)
                .bind(now)
                .bind(now)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?;
            }
            Backend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO workspaces (id, name, description, created_at, updated_at, deleted_at)
                     VALUES ($1, $2, $3, $4, $5, NULL);",
                )
                .bind(&id)
                .bind(name)
                .bind(description)
                .bind(now)
                .bind(now)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?;
            }
        }

        Ok(Workspace {
            id,
            name: name.to_string(),
            description: description.map(|value| value.to_string()),
            created_at: now,
            updated_at: now,
            deleted_at: None,
        })
    }

    pub async fn list_workspaces(&self) -> Result<Vec<Workspace>, String> {
        match &self.backend {
            Backend::Sqlite(pool) => {
                let rows = sqlx::query(
                    "SELECT id, name, description, created_at, updated_at, deleted_at
                     FROM workspaces
                     WHERE deleted_at IS NULL
                     ORDER BY created_at ASC;",
                )
                .fetch_all(pool)
                .await
                .map_err(|err| err.to_string())?;
                rows.into_iter().map(workspace_from_sqlite).collect()
            }
            Backend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, name, description, created_at, updated_at, deleted_at
                     FROM workspaces
                     WHERE deleted_at IS NULL
                     ORDER BY created_at ASC;",
                )
                .fetch_all(pool)
                .await
                .map_err(|err| err.to_string())?;
                rows.into_iter().map(workspace_from_postgres).collect()
            }
        }
    }

    pub async fn fetch_workspace_by_name(&self, name: &str) -> Result<Option<Workspace>, String> {
        match &self.backend {
            Backend::Sqlite(pool) => {
                let row = sqlx::query(
                    "SELECT id, name, description, created_at, updated_at, deleted_at
                     FROM workspaces
                     WHERE name = ? AND deleted_at IS NULL;",
                )
                .bind(name)
                .fetch_optional(pool)
                .await
                .map_err(|err| err.to_string())?;
                match row {
                    Some(row) => Ok(Some(workspace_from_sqlite(row)?)),
                    None => Ok(None),
                }
            }
            Backend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, name, description, created_at, updated_at, deleted_at
                     FROM workspaces
                     WHERE name = $1 AND deleted_at IS NULL;",
                )
                .bind(name)
                .fetch_optional(pool)
                .await
                .map_err(|err| err.to_string())?;
                match row {
                    Some(row) => Ok(Some(workspace_from_postgres(row)?)),
                    None => Ok(None),
                }
            }
        }
    }

    pub async fn update_workspace(
        &self,
        current_name: &str,
        name: &str,
        description: Option<&str>,
    ) -> Result<Option<Workspace>, String> {
        if current_name != name && self.workspace_exists_by_name(name).await? {
            return Err("Workspace already exists".to_string());
        }
        let now = now_millis();
        let affected = match &self.backend {
            Backend::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE workspaces
                     SET name = ?, description = ?, updated_at = ?
                     WHERE name = ? AND deleted_at IS NULL;",
                )
                .bind(name)
                .bind(description)
                .bind(now)
                .bind(current_name)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?
                .rows_affected()
            }
            Backend::Postgres(pool) => {
                sqlx::query(
                    "UPDATE workspaces
                     SET name = $1, description = $2, updated_at = $3
                     WHERE name = $4 AND deleted_at IS NULL;",
                )
                .bind(name)
                .bind(description)
                .bind(now)
                .bind(current_name)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?
                .rows_affected()
            }
        };

        if affected == 0 {
            return Ok(None);
        }

        self.fetch_workspace_by_name(name).await
    }

    pub async fn delete_workspace(&self, name: &str) -> Result<bool, String> {
        let now = now_millis();
        let affected = match &self.backend {
            Backend::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE workspaces
                     SET deleted_at = ?, updated_at = ?
                     WHERE name = ? AND deleted_at IS NULL;",
                )
                .bind(now)
                .bind(now)
                .bind(name)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?
                .rows_affected()
            }
            Backend::Postgres(pool) => {
                sqlx::query(
                    "UPDATE workspaces
                     SET deleted_at = $1, updated_at = $2
                     WHERE name = $3 AND deleted_at IS NULL;",
                )
                .bind(now)
                .bind(now)
                .bind(name)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?
                .rows_affected()
            }
        };
        Ok(affected > 0)
    }

    pub async fn workspace_exists_by_name(&self, name: &str) -> Result<bool, String> {
        let exists = match &self.backend {
            Backend::Sqlite(pool) => {
                sqlx::query("SELECT 1 FROM workspaces WHERE name = ?;")
                    .bind(name)
                    .fetch_optional(pool)
                    .await
                    .map_err(|err| err.to_string())?
                    .is_some()
            }
            Backend::Postgres(pool) => {
                sqlx::query("SELECT 1 FROM workspaces WHERE name = $1;")
                    .bind(name)
                    .fetch_optional(pool)
                    .await
                    .map_err(|err| err.to_string())?
                    .is_some()
            }
        };
        Ok(exists)
    }

    pub(super) async fn ensure_default_workspace(&self) -> Result<(), String> {
        let count = match &self.backend {
            Backend::Sqlite(pool) => {
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workspaces;")
                    .fetch_one(pool)
                    .await
                    .map_err(|err| err.to_string())?
            }
            Backend::Postgres(pool) => {
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workspaces;")
                    .fetch_one(pool)
                    .await
                    .map_err(|err| err.to_string())?
            }
        };

        if count == 0 {
            self.create_workspace(DEFAULT_WORKSPACE_NAME, None).await?;
        }

        Ok(())
    }
}
