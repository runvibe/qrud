use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::models::{Document, Workspace, WorkspaceOutput};
use crate::services::{AppState, DEFAULT_WORKSPACE_NAME};

pub(crate) struct DocumentSelector {
    pub(crate) pk: String,
    pub(crate) id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ListParams {
    pub(crate) limit: Option<i64>,
    pub(crate) offset: Option<i64>,
    pub(crate) term: Option<String>,
    pub(crate) order: Option<String>,
    pub(crate) by: Option<String>,
}

pub(crate) fn parse_document_selector(raw: &str) -> Result<DocumentSelector, Response> {
    let normalized = normalize_pk(raw);
    let trimmed = normalized.trim_matches('/');
    if trimmed.is_empty() {
        let pk = validate_pk(&normalized)?;
        return Ok(DocumentSelector { pk, id: None });
    }

    let segments: Vec<&str> = trimmed.split('/').collect();
    let last = segments.last().copied().unwrap_or_default();
    if Uuid::parse_str(last).is_ok() {
        let parent_segments = &segments[..segments.len().saturating_sub(1)];
        let parent = if parent_segments.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", parent_segments.join("/"))
        };
        let pk = validate_pk(&parent)?;
        return Ok(DocumentSelector {
            pk,
            id: Some(last.to_string()),
        });
    }

    let pk = validate_pk(&normalized)?;
    Ok(DocumentSelector { pk, id: None })
}

pub(crate) fn validate_contract_route(
    state: &AppState,
    method: &str,
    selector: &DocumentSelector,
) -> Result<(), Response> {
    let Some(contract) = state.api_contract() else {
        return Ok(());
    };

    let request_path = selector_request_path(selector);
    if contract.validate_route(method, &request_path) {
        return Ok(());
    }
    Err(json_error(
        StatusCode::NOT_FOUND,
        "Path not found in OpenAPI contract",
    ))
}

pub(crate) fn validate_contract_payload(
    state: &AppState,
    method: &str,
    selector: &DocumentSelector,
    payload: &JsonValue,
) -> Result<(), Response> {
    let Some(contract) = state.api_contract() else {
        return Ok(());
    };

    let request_path = selector_request_path(selector);
    match contract.validate_payload(method, &request_path, payload) {
        Ok(()) => Ok(()),
        Err(message) if message == "Route not found in OpenAPI" => Err(json_error(
            StatusCode::NOT_FOUND,
            "Path not found in OpenAPI contract",
        )),
        Err(message) => Err(json_error(StatusCode::BAD_REQUEST, &message)),
    }
}

pub(crate) async fn ensure_workspace(
    state: &AppState,
    workspace: &str,
) -> Result<Workspace, Response> {
    let name = workspace.trim();
    if name.is_empty() || !is_dash_case(name) {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "Workspace name must be dash-case",
        ));
    }
    match state.store.fetch_workspace_by_name(name).await {
        Ok(Some(workspace)) => Ok(workspace),
        Ok(None) => Err(json_error(StatusCode::NOT_FOUND, "Workspace not found")),
        Err(message) => Err(json_error(StatusCode::INTERNAL_SERVER_ERROR, &message)),
    }
}

pub(crate) fn workspace_from_header(
    headers: &HeaderMap,
    use_default: bool,
) -> Result<String, Response> {
    let header = headers.get("x-workspace-id");
    if header.is_none() {
        if use_default {
            return Ok(DEFAULT_WORKSPACE_NAME.to_string());
        }
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "x-workspace-id header required",
        ));
    }
    let value = header
        .unwrap()
        .to_str()
        .map_err(|_| json_error(StatusCode::BAD_REQUEST, "Invalid x-workspace-id header"))?;
    let value = value.trim();
    if value.is_empty() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "x-workspace-id header required",
        ));
    }
    if !is_dash_case(value) {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "Workspace name must be dash-case",
        ));
    }
    Ok(value.to_string())
}

pub(crate) fn document_to_output(document: Document) -> JsonValue {
    let mut output = serde_json::Map::new();
    output.insert("$id".to_string(), JsonValue::String(document.id));
    output.insert(
        "$createdAt".to_string(),
        JsonValue::String(format_millis_iso(document.created_at)),
    );
    output.insert(
        "$updatedAt".to_string(),
        JsonValue::String(format_millis_iso(document.updated_at)),
    );
    if let Some(deleted_at) = document.deleted_at {
        output.insert(
            "$deletedAt".to_string(),
            JsonValue::String(format_millis_iso(deleted_at)),
        );
    }

    match document.data {
        JsonValue::Object(map) => {
            for (key, value) in map {
                if matches!(
                    key.as_str(),
                    "id" | "workspace_id" | "pk" | "created_at" | "updated_at" | "deleted_at"
                ) {
                    continue;
                }
                output.insert(key, value);
            }
        }
        value => {
            output.insert("value".to_string(), value);
        }
    }

    JsonValue::Object(output)
}

pub(crate) fn workspace_to_output(workspace: Workspace) -> WorkspaceOutput {
    WorkspaceOutput {
        id: workspace.id,
        name: workspace.name,
        description: workspace.description,
        created_at: format_millis_iso(workspace.created_at),
        updated_at: format_millis_iso(workspace.updated_at),
        deleted_at: workspace.deleted_at.map(format_millis_iso),
    }
}

pub(crate) fn json_error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

pub(crate) fn is_dash_case(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !is_dash_char(first, false) {
        return false;
    }
    let mut prev_dash = first == '-';
    for ch in chars {
        if !is_dash_char(ch, true) {
            return false;
        }
        if ch == '-' {
            if prev_dash {
                return false;
            }
            prev_dash = true;
        } else {
            prev_dash = false;
        }
    }
    !prev_dash
}

fn normalize_pk(pk: &str) -> String {
    let trimmed = pk.trim();
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

fn validate_pk(pk: &str) -> Result<String, Response> {
    let normalized = normalize_pk(pk);
    if is_reserved_pk(&normalized) {
        return Err(json_error(StatusCode::BAD_REQUEST, "PK is reserved"));
    }
    Ok(normalized)
}

fn selector_request_path(selector: &DocumentSelector) -> String {
    if let Some(id) = selector.id.as_deref() {
        format!("{}/{}", selector.pk.trim_end_matches('/'), id)
    } else {
        selector.pk.clone()
    }
}

fn is_reserved_pk(pk: &str) -> bool {
    let trimmed = pk.trim_end_matches('/');
    let value = trimmed.strip_prefix('/').unwrap_or(trimmed);
    if value.is_empty() || value.contains('/') {
        return false;
    }
    matches!(
        value.to_ascii_lowercase().as_str(),
        "health" | "heath" | "info" | "workspaces" | "documents"
    )
}

fn format_millis_iso(millis: i64) -> String {
    let nanos = i128::from(millis) * 1_000_000;
    let datetime = OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .unwrap_or_else(|_| OffsetDateTime::from_unix_timestamp(0).unwrap());
    datetime
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn is_dash_char(ch: char, allow_dash: bool) -> bool {
    match ch {
        'a'..='z' | '0'..='9' => true,
        '-' => allow_dash,
        _ => false,
    }
}
