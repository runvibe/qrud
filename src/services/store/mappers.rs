use sqlx::Row;

use crate::models::{Document, Workspace};

pub(super) fn workspace_from_sqlite(row: sqlx::sqlite::SqliteRow) -> Result<Workspace, String> {
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

pub(super) fn workspace_from_postgres(
    row: sqlx::postgres::PgRow,
) -> Result<Workspace, String> {
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

pub(super) fn document_from_sqlite(row: sqlx::sqlite::SqliteRow) -> Result<Document, String> {
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

pub(super) fn document_from_postgres(
    row: sqlx::postgres::PgRow,
) -> Result<Document, String> {
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
