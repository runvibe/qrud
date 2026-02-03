use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{PgPool, Row, SqlitePool};
use uuid::Uuid;

use crate::models::{Document, Workspace};

#[derive(Clone)]
enum Backend {
    Sqlite(SqlitePool),
    Postgres(PgPool),
}

#[derive(Clone)]
pub struct Store {
    backend: Backend,
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
        Ok(Self {
            backend: Backend::Sqlite(pool),
        })
    }

    pub async fn open_postgres(url: &str) -> Result<Self, String> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(url)
            .await
            .map_err(|err| err.to_string())?;
        migrate_postgres(&pool).await?;
        Ok(Self {
            backend: Backend::Postgres(pool),
        })
    }

    pub async fn create_workspace(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<Workspace, String> {
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

    pub async fn fetch_workspace(&self, id: &str) -> Result<Option<Workspace>, String> {
        match &self.backend {
            Backend::Sqlite(pool) => {
                let row = sqlx::query(
                    "SELECT id, name, description, created_at, updated_at, deleted_at
                     FROM workspaces
                     WHERE id = ? AND deleted_at IS NULL;",
                )
                .bind(id)
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
                     WHERE id = $1 AND deleted_at IS NULL;",
                )
                .bind(id)
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
        id: &str,
        name: &str,
        description: Option<&str>,
    ) -> Result<Option<Workspace>, String> {
        let now = now_millis();
        let affected = match &self.backend {
            Backend::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE workspaces
                     SET name = ?, description = ?, updated_at = ?
                     WHERE id = ? AND deleted_at IS NULL;",
                )
                .bind(name)
                .bind(description)
                .bind(now)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?
                .rows_affected()
            }
            Backend::Postgres(pool) => {
                sqlx::query(
                    "UPDATE workspaces
                     SET name = $1, description = $2, updated_at = $3
                     WHERE id = $4 AND deleted_at IS NULL;",
                )
                .bind(name)
                .bind(description)
                .bind(now)
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

        self.fetch_workspace(id).await
    }

    pub async fn delete_workspace(&self, id: &str) -> Result<bool, String> {
        let now = now_millis();
        let affected = match &self.backend {
            Backend::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE workspaces
                     SET deleted_at = ?, updated_at = ?
                     WHERE id = ? AND deleted_at IS NULL;",
                )
                .bind(now)
                .bind(now)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?
                .rows_affected()
            }
            Backend::Postgres(pool) => {
                sqlx::query(
                    "UPDATE workspaces
                     SET deleted_at = $1, updated_at = $2
                     WHERE id = $3 AND deleted_at IS NULL;",
                )
                .bind(now)
                .bind(now)
                .bind(id)
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
        if self
            .fetch_document_including_deleted(workspace_id, pk)
            .await?
            .is_some()
        {
            return Err("Document already exists".to_string());
        }

        let id = new_uuid();
        let now = now_millis();
        let data = serde_json::to_string(data).map_err(|err| err.to_string())?;

        match &self.backend {
            Backend::Sqlite(pool) => {
                sqlx::query(
                    "INSERT INTO documents (id, workspace_id, pk, data, created_at, updated_at, deleted_at)
                     VALUES (?, ?, ?, ?, ?, ?, NULL);",
                )
                .bind(&id)
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
                .bind(&id)
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

        Ok(Document {
            id,
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

    pub async fn update_document_data(
        &self,
        workspace_id: &str,
        pk: &str,
        data: &serde_json::Value,
    ) -> Result<Option<Document>, String> {
        let now = now_millis();
        let data = serde_json::to_string(data).map_err(|err| err.to_string())?;

        let affected = match &self.backend {
            Backend::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE documents
                     SET data = ?, updated_at = ?, deleted_at = NULL
                     WHERE workspace_id = ? AND pk = ?;",
                )
                .bind(&data)
                .bind(now)
                .bind(workspace_id)
                .bind(pk)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?
                .rows_affected()
            }
            Backend::Postgres(pool) => {
                sqlx::query(
                    "UPDATE documents
                     SET data = $1, updated_at = $2, deleted_at = NULL
                     WHERE workspace_id = $3 AND pk = $4;",
                )
                .bind(&data)
                .bind(now)
                .bind(workspace_id)
                .bind(pk)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?
                .rows_affected()
            }
        };

        if affected == 0 {
            return Ok(None);
        }

        self.fetch_document(workspace_id, pk).await
    }

    pub async fn fetch_document(
        &self,
        workspace_id: &str,
        pk: &str,
    ) -> Result<Option<Document>, String> {
        self.fetch_document_row(workspace_id, pk, false).await
    }

    pub async fn delete_document(&self, workspace_id: &str, pk: &str) -> Result<bool, String> {
        let now = now_millis();
        let affected = match &self.backend {
            Backend::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE documents
                     SET deleted_at = ?, updated_at = ?
                     WHERE workspace_id = ? AND pk = ? AND deleted_at IS NULL;",
                )
                .bind(now)
                .bind(now)
                .bind(workspace_id)
                .bind(pk)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?
                .rows_affected()
            }
            Backend::Postgres(pool) => {
                sqlx::query(
                    "UPDATE documents
                     SET deleted_at = $1, updated_at = $2
                     WHERE workspace_id = $3 AND pk = $4 AND deleted_at IS NULL;",
                )
                .bind(now)
                .bind(now)
                .bind(workspace_id)
                .bind(pk)
                .execute(pool)
                .await
                .map_err(|err| err.to_string())?
                .rows_affected()
            }
        };

        Ok(affected > 0)
    }

    pub async fn workspace_exists(&self, id: &str) -> Result<bool, String> {
        let exists = match &self.backend {
            Backend::Sqlite(pool) => {
                sqlx::query("SELECT 1 FROM workspaces WHERE id = ? AND deleted_at IS NULL;")
                    .bind(id)
                    .fetch_optional(pool)
                    .await
                    .map_err(|err| err.to_string())?
                    .is_some()
            }
            Backend::Postgres(pool) => {
                sqlx::query("SELECT 1 FROM workspaces WHERE id = $1 AND deleted_at IS NULL;")
                    .bind(id)
                    .fetch_optional(pool)
                    .await
                    .map_err(|err| err.to_string())?
                    .is_some()
            }
        };
        Ok(exists)
    }

    async fn fetch_document_including_deleted(
        &self,
        workspace_id: &str,
        pk: &str,
    ) -> Result<Option<Document>, String> {
        self.fetch_document_row(workspace_id, pk, true).await
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
                     FROM documents WHERE workspace_id = ? AND pk = ?;"
                } else {
                    "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                     FROM documents WHERE workspace_id = ? AND pk = ? AND deleted_at IS NULL;"
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
                     FROM documents WHERE workspace_id = $1 AND pk = $2;"
                } else {
                    "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                     FROM documents WHERE workspace_id = $1 AND pk = $2 AND deleted_at IS NULL;"
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
