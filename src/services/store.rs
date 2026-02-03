use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{PgPool, Row, SqlitePool};
use url::Url;
use uuid::Uuid;

use crate::models::{Document, Workspace};
use crate::services::DEFAULT_WORKSPACE_NAME;

#[derive(Clone)]
enum Backend {
    Sqlite(SqlitePool),
    Postgres(PgPool),
}

#[derive(Clone)]
pub struct Store {
    backend: Backend,
    info: DatabaseInfo,
}

#[derive(Clone, Debug, Serialize)]
pub struct DatabaseInfo {
    pub drive: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sqlite: Option<SqliteInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postgres: Option<PostgresInfo>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SqliteInfo {
    pub in_memory: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PostgresInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
}

impl Store {
    pub async fn open_sqlite(path: &str) -> Result<Self, String> {
        let options = if path == ":memory:" {
            SqliteConnectOptions::new()
                .in_memory(true)
                .shared_cache(true)
                .foreign_keys(true)
        } else {
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .foreign_keys(true)
        };

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|err| err.to_string())?;
        migrate_sqlite(&pool).await?;
        let info = DatabaseInfo {
            drive: "sqlite",
            sqlite: Some(SqliteInfo {
                in_memory: path == ":memory:",
                path: if path == ":memory:" {
                    None
                } else {
                    Some(path.to_string())
                },
            }),
            postgres: None,
        };
        let store = Self {
            backend: Backend::Sqlite(pool),
            info,
        };
        store.ensure_default_workspace().await?;
        Ok(store)
    }

    pub async fn open_postgres(url: &str) -> Result<Self, String> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(url)
            .await
            .map_err(|err| err.to_string())?;
        migrate_postgres(&pool).await?;
        let info = DatabaseInfo {
            drive: "postgres",
            sqlite: None,
            postgres: Some(parse_postgres_info(url)),
        };
        let store = Self {
            backend: Backend::Postgres(pool),
            info,
        };
        store.ensure_default_workspace().await?;
        Ok(store)
    }

    pub fn database_info(&self) -> &DatabaseInfo {
        &self.info
    }

    pub async fn create_workspace(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<Workspace, String> {
        if self.workspace_exists_by_name(name).await? {
            return Err("Workspace already exists".to_string());
        }
        let id = new_uuid();
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

    pub async fn create_document(
        &self,
        workspace_id: &str,
        pk: &str,
        data: &serde_json::Value,
    ) -> Result<Document, String> {
        let id = new_uuid();
        self.insert_document(workspace_id, &id, pk, data).await
    }

    pub async fn create_document_with_id(
        &self,
        workspace_id: &str,
        id: &str,
        pk: &str,
        data: &serde_json::Value,
    ) -> Result<Document, String> {
        self.insert_document(workspace_id, id, pk, data).await
    }

    async fn insert_document(
        &self,
        workspace_id: &str,
        id: &str,
        pk: &str,
        data: &serde_json::Value,
    ) -> Result<Document, String> {
        let now = now_millis();
        let data = serde_json::to_string(data).map_err(|err| err.to_string())?;

        match &self.backend {
            Backend::Sqlite(pool) => {
                sqlx::query(
                    "INSERT INTO documents (id, workspace_id, pk, data, created_at, updated_at, deleted_at)
                     VALUES (?, ?, ?, ?, ?, ?, NULL);",
                )
                .bind(id)
                .bind(workspace_id)
                .bind(pk)
                .bind(&data)
                .bind(now)
                .bind(now)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?;
            }
            Backend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO documents (id, workspace_id, pk, data, created_at, updated_at, deleted_at)
                     VALUES ($1, $2, $3, $4, $5, $6, NULL);",
                )
                .bind(id)
                .bind(workspace_id)
                .bind(pk)
                .bind(&data)
                .bind(now)
                .bind(now)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?;
            }
        }

        self.increment_meta_pk(workspace_id, pk).await?;

        Ok(Document {
            id: id.to_string(),
            workspace_id: workspace_id.to_string(),
            pk: pk.to_string(),
            data: serde_json::from_str(&data).map_err(|err| err.to_string())?,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        })
    }

    pub async fn upsert_document(
        &self,
        workspace_id: &str,
        pk: &str,
        data: &serde_json::Value,
    ) -> Result<(bool, Document), String> {
        let existing = self
            .fetch_document_including_deleted(workspace_id, pk)
            .await?;

        if let Some(doc) = existing {
            let updated = self
                .update_document_data(workspace_id, pk, data)
                .await?
                .unwrap_or(doc);
            return Ok((false, updated));
        }

        let created = self.create_document(workspace_id, pk, data).await?;
        Ok((true, created))
    }

    pub async fn upsert_document_by_id(
        &self,
        workspace_id: &str,
        id: &str,
        pk: &str,
        data: &serde_json::Value,
    ) -> Result<(bool, Document), String> {
        let existing = self
            .fetch_document_including_deleted_by_id(workspace_id, id)
            .await?;

        if let Some(doc) = existing {
            let updated = self
                .update_document_data_by_id(workspace_id, id, data)
                .await?
                .unwrap_or(doc);
            return Ok((false, updated));
        }

        let created = self.create_document_with_id(workspace_id, id, pk, data).await?;
        Ok((true, created))
    }

    pub async fn update_document_data(
        &self,
        workspace_id: &str,
        pk: &str,
        data: &serde_json::Value,
    ) -> Result<Option<Document>, String> {
        let existing = self.fetch_document(workspace_id, pk).await?;
        let Some(doc) = existing else {
            return Ok(None);
        };
        self.update_document_data_by_id(workspace_id, &doc.id, data)
            .await
    }

    pub async fn update_document_data_by_id(
        &self,
        workspace_id: &str,
        id: &str,
        data: &serde_json::Value,
    ) -> Result<Option<Document>, String> {
        let existing = self
            .fetch_document_including_deleted_by_id(workspace_id, id)
            .await?;
        let Some(existing) = existing else {
            return Ok(None);
        };

        let now = now_millis();
        let data = serde_json::to_string(data).map_err(|err| err.to_string())?;

        let affected = match &self.backend {
            Backend::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE documents
                     SET data = ?, updated_at = ?, deleted_at = NULL
                     WHERE workspace_id = ? AND id = ?;",
                )
                .bind(&data)
                .bind(now)
                .bind(workspace_id)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?
                .rows_affected()
            }
            Backend::Postgres(pool) => {
                sqlx::query(
                    "UPDATE documents
                     SET data = $1, updated_at = $2, deleted_at = NULL
                     WHERE workspace_id = $3 AND id = $4;",
                )
                .bind(&data)
                .bind(now)
                .bind(workspace_id)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?
                .rows_affected()
            }
        };

        if affected == 0 {
            return Ok(None);
        }

        let updated = self.fetch_document_by_id(workspace_id, id).await?;
        if existing.deleted_at.is_some() {
            self.increment_meta_pk(workspace_id, &existing.pk).await?;
        }
        Ok(updated)
    }

    pub async fn fetch_document(
        &self,
        workspace_id: &str,
        pk: &str,
    ) -> Result<Option<Document>, String> {
        self.fetch_document_row(workspace_id, pk, false).await
    }

    pub async fn fetch_documents_by_pk(
        &self,
        workspace_id: &str,
        pk: &str,
        term: Option<&str>,
        limit: Option<i64>,
        offset: i64,
    ) -> Result<Vec<Document>, String> {
        let offset = offset.max(0);
        let term = term.map(|value| format!("%{}%", value));
        match &self.backend {
            Backend::Sqlite(pool) => {
                let (sql, bind_limit, bind_offset) = if term.is_some() {
                    (
                        "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                         FROM documents
                         WHERE workspace_id = ? AND pk = ? AND deleted_at IS NULL AND (
                           lower(coalesce(json_extract(data, '$.name'), '')) LIKE ?
                           OR lower(coalesce(json_extract(data, '$.title'), '')) LIKE ?
                           OR lower(coalesce(json_extract(data, '$.label'), '')) LIKE ?
                           OR lower(coalesce(json_extract(data, '$.reference'), '')) LIKE ?
                           OR lower(coalesce(json_extract(data, '$.category'), '')) LIKE ?
                           OR lower(coalesce(json_extract(data, '$.description'), '')) LIKE ?
                         )
                         ORDER BY updated_at DESC
                         LIMIT ? OFFSET ?;",
                        true,
                        true,
                    )
                } else if limit.is_some() {
                    (
                        "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                         FROM documents
                         WHERE workspace_id = ? AND pk = ? AND deleted_at IS NULL
                         ORDER BY updated_at DESC
                         LIMIT ? OFFSET ?;",
                        true,
                        true,
                    )
                } else {
                    (
                        "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                         FROM documents
                         WHERE workspace_id = ? AND pk = ? AND deleted_at IS NULL
                         ORDER BY updated_at DESC
                         LIMIT -1 OFFSET ?;",
                        false,
                        true,
                    )
                };

                let mut query = sqlx::query(sql).bind(workspace_id).bind(pk);
                if let Some(term) = term.as_ref() {
                    for _ in 0..6 {
                        query = query.bind(term);
                    }
                }
                if bind_limit {
                    query = query.bind(limit.unwrap_or(i64::MAX));
                }
                if bind_offset {
                    query = query.bind(offset);
                }
                let rows = query
                    .fetch_all(pool)
                    .await
                    .map_err(|err| err.to_string())?;
                rows.into_iter()
                    .map(document_from_sqlite)
                    .collect::<Result<Vec<_>, _>>()
            }
            Backend::Postgres(pool) => {
                let (sql, bind_limit, bind_offset) = if term.is_some() {
                    (
                        "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                         FROM documents
                         WHERE workspace_id = $1 AND pk = $2 AND deleted_at IS NULL AND (
                           lower(coalesce(data::jsonb->>'name', '')) LIKE $3
                           OR lower(coalesce(data::jsonb->>'title', '')) LIKE $4
                           OR lower(coalesce(data::jsonb->>'label', '')) LIKE $5
                           OR lower(coalesce(data::jsonb->>'reference', '')) LIKE $6
                           OR lower(coalesce(data::jsonb->>'category', '')) LIKE $7
                           OR lower(coalesce(data::jsonb->>'description', '')) LIKE $8
                         )
                         ORDER BY updated_at DESC
                         LIMIT $9 OFFSET $10;",
                        true,
                        true,
                    )
                } else if limit.is_some() {
                    (
                        "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                         FROM documents
                         WHERE workspace_id = $1 AND pk = $2 AND deleted_at IS NULL
                         ORDER BY updated_at DESC
                         LIMIT $3 OFFSET $4;",
                        true,
                        true,
                    )
                } else {
                    (
                        "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                         FROM documents
                         WHERE workspace_id = $1 AND pk = $2 AND deleted_at IS NULL
                         ORDER BY updated_at DESC
                         OFFSET $3;",
                        false,
                        true,
                    )
                };

                let mut query = sqlx::query(sql).bind(workspace_id).bind(pk);
                if let Some(term) = term.as_ref() {
                    for _ in 0..6 {
                        query = query.bind(term);
                    }
                }
                if bind_limit {
                    query = query.bind(limit.unwrap_or(i64::MAX));
                }
                if bind_offset {
                    query = query.bind(offset);
                }
                let rows = query
                    .fetch_all(pool)
                    .await
                    .map_err(|err| err.to_string())?;
                rows.into_iter()
                    .map(document_from_postgres)
                    .collect::<Result<Vec<_>, _>>()
            }
        }
    }

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

    pub async fn fetch_document_by_id(
        &self,
        workspace_id: &str,
        id: &str,
    ) -> Result<Option<Document>, String> {
        self.fetch_document_row_by_id(workspace_id, id, false)
            .await
    }

    pub async fn delete_document(&self, workspace_id: &str, pk: &str) -> Result<bool, String> {
        let existing = self.fetch_document(workspace_id, pk).await?;
        let Some(doc) = existing else {
            return Ok(false);
        };
        self.delete_document_by_id(workspace_id, &doc.id).await
    }

    pub async fn delete_document_by_id(
        &self,
        workspace_id: &str,
        id: &str,
    ) -> Result<bool, String> {
        let existing = self
            .fetch_document_by_id(workspace_id, id)
            .await?;
        let Some(existing) = existing else {
            return Ok(false);
        };

        let now = now_millis();
        let affected = match &self.backend {
            Backend::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE documents
                     SET deleted_at = ?, updated_at = ?
                     WHERE workspace_id = ? AND id = ? AND deleted_at IS NULL;",
                )
                .bind(now)
                .bind(now)
                .bind(workspace_id)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?
                .rows_affected()
            }
            Backend::Postgres(pool) => {
                sqlx::query(
                    "UPDATE documents
                     SET deleted_at = $1, updated_at = $2
                     WHERE workspace_id = $3 AND id = $4 AND deleted_at IS NULL;",
                )
                .bind(now)
                .bind(now)
                .bind(workspace_id)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?
                .rows_affected()
            }
        };

        if affected > 0 {
            self.decrement_meta_pk(workspace_id, &existing.pk).await?;
            return Ok(true);
        }

        Ok(false)
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

    async fn ensure_default_workspace(&self) -> Result<(), String> {
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

    async fn fetch_document_including_deleted(
        &self,
        workspace_id: &str,
        pk: &str,
    ) -> Result<Option<Document>, String> {
        self.fetch_document_row(workspace_id, pk, true).await
    }

    async fn fetch_document_including_deleted_by_id(
        &self,
        workspace_id: &str,
        id: &str,
    ) -> Result<Option<Document>, String> {
        self.fetch_document_row_by_id(workspace_id, id, true).await
    }

    async fn fetch_document_row(
        &self,
        workspace_id: &str,
        pk: &str,
        include_deleted: bool,
    ) -> Result<Option<Document>, String> {
        match &self.backend {
            Backend::Sqlite(pool) => {
                let sql = if include_deleted {
                    "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                     FROM documents
                     WHERE workspace_id = ? AND pk = ?
                     ORDER BY updated_at DESC
                     LIMIT 1;"
                } else {
                    "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                     FROM documents
                     WHERE workspace_id = ? AND pk = ? AND deleted_at IS NULL
                     ORDER BY updated_at DESC
                     LIMIT 1;"
                };
                let row = sqlx::query(sql)
                    .bind(workspace_id)
                    .bind(pk)
                    .fetch_optional(pool)
                    .await
                    .map_err(|err| err.to_string())?;
                match row {
                    Some(row) => Ok(Some(document_from_sqlite(row)?)),
                    None => Ok(None),
                }
            }
            Backend::Postgres(pool) => {
                let sql = if include_deleted {
                    "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                     FROM documents
                     WHERE workspace_id = $1 AND pk = $2
                     ORDER BY updated_at DESC
                     LIMIT 1;"
                } else {
                    "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                     FROM documents
                     WHERE workspace_id = $1 AND pk = $2 AND deleted_at IS NULL
                     ORDER BY updated_at DESC
                     LIMIT 1;"
                };
                let row = sqlx::query(sql)
                    .bind(workspace_id)
                    .bind(pk)
                    .fetch_optional(pool)
                    .await
                    .map_err(|err| err.to_string())?;
                match row {
                    Some(row) => Ok(Some(document_from_postgres(row)?)),
                    None => Ok(None),
                }
            }
        }
    }

    async fn fetch_document_row_by_id(
        &self,
        workspace_id: &str,
        id: &str,
        include_deleted: bool,
    ) -> Result<Option<Document>, String> {
        match &self.backend {
            Backend::Sqlite(pool) => {
                let sql = if include_deleted {
                    "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                     FROM documents WHERE workspace_id = ? AND id = ?;"
                } else {
                    "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                     FROM documents WHERE workspace_id = ? AND id = ? AND deleted_at IS NULL;"
                };
                let row = sqlx::query(sql)
                    .bind(workspace_id)
                    .bind(id)
                    .fetch_optional(pool)
                    .await
                    .map_err(|err| err.to_string())?;
                match row {
                    Some(row) => Ok(Some(document_from_sqlite(row)?)),
                    None => Ok(None),
                }
            }
            Backend::Postgres(pool) => {
                let sql = if include_deleted {
                    "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                     FROM documents WHERE workspace_id = $1 AND id = $2;"
                } else {
                    "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                     FROM documents WHERE workspace_id = $1 AND id = $2 AND deleted_at IS NULL;"
                };
                let row = sqlx::query(sql)
                    .bind(workspace_id)
                    .bind(id)
                    .fetch_optional(pool)
                    .await
                    .map_err(|err| err.to_string())?;
                match row {
                    Some(row) => Ok(Some(document_from_postgres(row)?)),
                    None => Ok(None),
                }
            }
        }
    }

    async fn increment_meta_pk(&self, workspace_id: &str, pk: &str) -> Result<(), String> {
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

    async fn decrement_meta_pk(&self, workspace_id: &str, pk: &str) -> Result<(), String> {
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

fn parse_postgres_info(url: &str) -> PostgresInfo {
    let parsed = Url::parse(url).ok();
    let host = parsed
        .as_ref()
        .and_then(|value| value.host_str())
        .map(|value| value.to_string());
    let port = parsed.as_ref().and_then(|value| value.port());
    let database = parsed
        .as_ref()
        .and_then(|value| value.path_segments())
        .and_then(|mut segments| segments.next_back())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    PostgresInfo {
        host,
        port,
        database,
    }
}

fn workspace_from_sqlite(row: sqlx::sqlite::SqliteRow) -> Result<Workspace, String> {
    let id: String = row.try_get("id").map_err(|err| err.to_string())?;
    let name: String = row.try_get("name").map_err(|err| err.to_string())?;
    let description: Option<String> = row.try_get("description").map_err(|err| err.to_string())?;
    let created_at: i64 = row.try_get("created_at").map_err(|err| err.to_string())?;
    let updated_at: i64 = row.try_get("updated_at").map_err(|err| err.to_string())?;
    let deleted_at: Option<i64> = row.try_get("deleted_at").map_err(|err| err.to_string())?;

    Ok(Workspace {
        id,
        name,
        description,
        created_at,
        updated_at,
        deleted_at,
    })
}

fn workspace_from_postgres(row: sqlx::postgres::PgRow) -> Result<Workspace, String> {
    let id: String = row.try_get("id").map_err(|err| err.to_string())?;
    let name: String = row.try_get("name").map_err(|err| err.to_string())?;
    let description: Option<String> = row.try_get("description").map_err(|err| err.to_string())?;
    let created_at: i64 = row.try_get("created_at").map_err(|err| err.to_string())?;
    let updated_at: i64 = row.try_get("updated_at").map_err(|err| err.to_string())?;
    let deleted_at: Option<i64> = row.try_get("deleted_at").map_err(|err| err.to_string())?;

    Ok(Workspace {
        id,
        name,
        description,
        created_at,
        updated_at,
        deleted_at,
    })
}

fn document_from_sqlite(row: sqlx::sqlite::SqliteRow) -> Result<Document, String> {
    let id: String = row.try_get("id").map_err(|err| err.to_string())?;
    let workspace_id: String = row
        .try_get("workspace_id")
        .map_err(|err| err.to_string())?;
    let pk: String = row.try_get("pk").map_err(|err| err.to_string())?;
    let data: String = row.try_get("data").map_err(|err| err.to_string())?;
    let created_at: i64 = row.try_get("created_at").map_err(|err| err.to_string())?;
    let updated_at: i64 = row.try_get("updated_at").map_err(|err| err.to_string())?;
    let deleted_at: Option<i64> = row.try_get("deleted_at").map_err(|err| err.to_string())?;
    let data = serde_json::from_str(&data).map_err(|err| err.to_string())?;

    Ok(Document {
        id,
        workspace_id,
        pk,
        data,
        created_at,
        updated_at,
        deleted_at,
    })
}

fn document_from_postgres(row: sqlx::postgres::PgRow) -> Result<Document, String> {
    let id: String = row.try_get("id").map_err(|err| err.to_string())?;
    let workspace_id: String = row
        .try_get("workspace_id")
        .map_err(|err| err.to_string())?;
    let pk: String = row.try_get("pk").map_err(|err| err.to_string())?;
    let data: String = row.try_get("data").map_err(|err| err.to_string())?;
    let created_at: i64 = row.try_get("created_at").map_err(|err| err.to_string())?;
    let updated_at: i64 = row.try_get("updated_at").map_err(|err| err.to_string())?;
    let deleted_at: Option<i64> = row.try_get("deleted_at").map_err(|err| err.to_string())?;
    let data = serde_json::from_str(&data).map_err(|err| err.to_string())?;

    Ok(Document {
        id,
        workspace_id,
        pk,
        data,
        created_at,
        updated_at,
        deleted_at,
    })
}

fn new_uuid() -> String {
    Uuid::now_v7().to_string()
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_millis() as i64
}

async fn migrate_sqlite(pool: &SqlitePool) -> Result<(), String> {
    sqlx::migrate!("./migrations/sqlite")
        .run(pool)
        .await
        .map_err(|err| err.to_string())
}

async fn migrate_postgres(pool: &PgPool) -> Result<(), String> {
    sqlx::migrate!("./migrations/postgres")
        .run(pool)
        .await
        .map_err(|err| err.to_string())
}
