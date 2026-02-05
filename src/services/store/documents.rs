use crate::models::Document;

use super::mappers::{document_from_postgres, document_from_sqlite};
use super::util::{new_uuid, now_millis};
use super::{Backend, Store};

impl Store {
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
        order_desc: bool,
        by: &str,
    ) -> Result<Vec<Document>, String> {
        let offset = offset.max(0);
        let term_exact = term.map(|value| value.to_string());
        let term_like = term.map(|value| format!("%{}%", value));
        let order_dir = if order_desc { "DESC" } else { "ASC" };
        let order_col = if by == "updated_at" {
            "updated_at"
        } else {
            "created_at"
        };
        match &self.backend {
            Backend::Sqlite(pool) => {
                let (sql, bind_limit, bind_offset, bind_term) = if term_exact.is_some() {
                    let sql = format!(
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
                         ORDER BY {order_col} {order_dir}
                         LIMIT ? OFFSET ?;",
                        order_col = order_col,
                        order_dir = order_dir
                    );
                    (sql, true, true, true)
                } else if limit.is_some() {
                    let sql = format!(
                        "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                         FROM documents
                         WHERE workspace_id = ? AND pk = ? AND deleted_at IS NULL
                         ORDER BY {order_col} {order_dir}
                         LIMIT ? OFFSET ?;",
                        order_col = order_col,
                        order_dir = order_dir
                    );
                    (sql, true, true, false)
                } else {
                    let sql = format!(
                        "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                         FROM documents
                         WHERE workspace_id = ? AND pk = ? AND deleted_at IS NULL
                         ORDER BY {order_col} {order_dir}
                         LIMIT -1 OFFSET ?;",
                        order_col = order_col,
                        order_dir = order_dir
                    );
                    (sql, false, true, false)
                };

                let mut query = sqlx::query(&sql).bind(workspace_id).bind(pk);
                if bind_term {
                    if let Some(term_like) = term_like.as_ref() {
                        for _ in 0..6 {
                            query = query.bind(term_like);
                        }
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
                let (sql, bind_limit, bind_offset, bind_term) = if term_exact.is_some() {
                    let sql = format!(
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
                         ORDER BY {order_col} {order_dir}
                         LIMIT $9 OFFSET $10;",
                        order_col = order_col,
                        order_dir = order_dir
                    );
                    (sql, true, true, true)
                } else if limit.is_some() {
                    let sql = format!(
                        "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                         FROM documents
                         WHERE workspace_id = $1 AND pk = $2 AND deleted_at IS NULL
                         ORDER BY {order_col} {order_dir}
                         LIMIT $3 OFFSET $4;",
                        order_col = order_col,
                        order_dir = order_dir
                    );
                    (sql, true, true, false)
                } else {
                    let sql = format!(
                        "SELECT id, workspace_id, pk, data, created_at, updated_at, deleted_at
                         FROM documents
                         WHERE workspace_id = $1 AND pk = $2 AND deleted_at IS NULL
                         ORDER BY {order_col} {order_dir}
                         OFFSET $3;",
                        order_col = order_col,
                        order_dir = order_dir
                    );
                    (sql, false, true, false)
                };

                let mut query = sqlx::query(&sql).bind(workspace_id).bind(pk);
                if bind_term {
                    if let Some(term_like) = term_like.as_ref() {
                        for _ in 0..6 {
                            query = query.bind(term_like);
                        }
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
        let existing = self.fetch_document_by_id(workspace_id, id).await?;
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
}
