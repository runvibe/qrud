use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value as JsonValue};

use crate::models::{AnyJson, Document, Workspace, WorkspaceInput, WorkspacePatch};
use crate::services::AppState;

#[utoipa::path(
    post,
    path = "/workspaces",
    request_body = WorkspaceInput,
    responses(
        (status = 201, body = Workspace, description = "Workspace criado")
    )
)]
pub(crate) async fn create_workspace(
    State(state): State<AppState>,
    Json(payload): Json<WorkspaceInput>,
) -> Response {
    if payload.name.trim().is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "Workspace name is required");
    }
    match state
        .store
        .create_workspace(&payload.name, payload.description.as_deref())
        .await
    {
        Ok(workspace) => (StatusCode::CREATED, Json(workspace)).into_response(),
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

#[utoipa::path(
    get,
    path = "/workspaces",
    responses(
        (status = 200, body = [Workspace], description = "Lista de workspaces")
    )
)]
pub(crate) async fn list_workspaces(State(state): State<AppState>) -> Response {
    match state.store.list_workspaces().await {
        Ok(workspaces) => (StatusCode::OK, Json(workspaces)).into_response(),
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

#[utoipa::path(
    get,
    path = "/workspaces/{workspace_id}",
    params(
        ("workspace_id" = String, Path, description = "ID do workspace")
    ),
    responses(
        (status = 200, body = Workspace, description = "Workspace encontrado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn get_workspace(
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Response {
    match state.store.fetch_workspace(&workspace_id).await {
        Ok(Some(workspace)) => (StatusCode::OK, Json(workspace)).into_response(),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "Workspace not found"),
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

#[utoipa::path(
    put,
    path = "/workspaces/{workspace_id}",
    request_body = WorkspaceInput,
    params(
        ("workspace_id" = String, Path, description = "ID do workspace")
    ),
    responses(
        (status = 200, body = Workspace, description = "Workspace atualizado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn put_workspace(
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(payload): Json<WorkspaceInput>,
) -> Response {
    if payload.name.trim().is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "Workspace name is required");
    }
    match state
        .store
        .update_workspace(&workspace_id, &payload.name, payload.description.as_deref())
        .await
    {
        Ok(Some(workspace)) => (StatusCode::OK, Json(workspace)).into_response(),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "Workspace not found"),
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

#[utoipa::path(
    patch,
    path = "/workspaces/{workspace_id}",
    request_body = WorkspacePatch,
    params(
        ("workspace_id" = String, Path, description = "ID do workspace")
    ),
    responses(
        (status = 200, body = Workspace, description = "Workspace atualizado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn patch_workspace(
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(payload): Json<WorkspacePatch>,
) -> Response {
    let mut workspace = match state.store.fetch_workspace(&workspace_id).await {
        Ok(Some(workspace)) => workspace,
        Ok(None) => return json_error(StatusCode::NOT_FOUND, "Workspace not found"),
        Err(message) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    };

    let mut changed = false;
    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return json_error(StatusCode::BAD_REQUEST, "Workspace name is required");
        }
        workspace.name = name;
        changed = true;
    }
    if let Some(description) = payload.description {
        workspace.description = Some(description);
        changed = true;
    }

    if !changed {
        return json_error(StatusCode::BAD_REQUEST, "No fields to update");
    }

    match state
        .store
        .update_workspace(&workspace_id, &workspace.name, workspace.description.as_deref())
        .await
    {
        Ok(Some(workspace)) => (StatusCode::OK, Json(workspace)).into_response(),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "Workspace not found"),
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

#[utoipa::path(
    delete,
    path = "/workspaces/{workspace_id}",
    params(
        ("workspace_id" = String, Path, description = "ID do workspace")
    ),
    responses(
        (status = 204, description = "Removido"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn delete_workspace(
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Response {
    match state.store.delete_workspace(&workspace_id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => json_error(StatusCode::NOT_FOUND, "Workspace not found"),
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

#[utoipa::path(
    post,
    path = "/workspaces/{workspace_id}/documents/{*pk}",
    request_body = AnyJson,
    params(
        ("workspace_id" = String, Path, description = "ID do workspace"),
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 201, body = Document, description = "Documento criado"),
        (status = 404, description = "Workspace nao encontrado"),
        (status = 409, description = "Documento ja existe")
    )
)]
pub(crate) async fn create_document(
    State(state): State<AppState>,
    Path((workspace_id, pk)): Path<(String, String)>,
    Json(payload): Json<JsonValue>,
) -> Response {
    document_create(state, workspace_id, pk, payload).await
}

#[utoipa::path(
    post,
    path = "/documents/{*pk}",
    request_body = AnyJson,
    params(
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 201, body = Document, description = "Documento criado"),
        (status = 400, description = "Workspace nao informado"),
        (status = 404, description = "Workspace nao encontrado"),
        (status = 409, description = "Documento ja existe")
    )
)]
pub(crate) async fn create_document_with_header(
    State(state): State<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<JsonValue>,
) -> Response {
    let workspace_id = match workspace_from_header(&headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    document_create(state, workspace_id, pk, payload).await
}

#[utoipa::path(
    get,
    path = "/workspaces/{workspace_id}/documents/{*pk}",
    params(
        ("workspace_id" = String, Path, description = "ID do workspace"),
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 200, body = Document, description = "Documento encontrado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn get_document(
    State(state): State<AppState>,
    Path((workspace_id, pk)): Path<(String, String)>,
) -> Response {
    document_get(state, workspace_id, pk).await
}

#[utoipa::path(
    get,
    path = "/documents/{*pk}",
    params(
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 200, body = Document, description = "Documento encontrado"),
        (status = 400, description = "Workspace nao informado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn get_document_with_header(
    State(state): State<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
) -> Response {
    let workspace_id = match workspace_from_header(&headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    document_get(state, workspace_id, pk).await
}

#[utoipa::path(
    put,
    path = "/workspaces/{workspace_id}/documents/{*pk}",
    request_body = AnyJson,
    params(
        ("workspace_id" = String, Path, description = "ID do workspace"),
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 200, body = Document, description = "Documento atualizado"),
        (status = 201, body = Document, description = "Documento criado"),
        (status = 404, description = "Workspace nao encontrado")
    )
)]
pub(crate) async fn put_document(
    State(state): State<AppState>,
    Path((workspace_id, pk)): Path<(String, String)>,
    Json(payload): Json<JsonValue>,
) -> Response {
    document_put(state, workspace_id, pk, payload).await
}

#[utoipa::path(
    put,
    path = "/documents/{*pk}",
    request_body = AnyJson,
    params(
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 200, body = Document, description = "Documento atualizado"),
        (status = 201, body = Document, description = "Documento criado"),
        (status = 400, description = "Workspace nao informado"),
        (status = 404, description = "Workspace nao encontrado")
    )
)]
pub(crate) async fn put_document_with_header(
    State(state): State<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<JsonValue>,
) -> Response {
    let workspace_id = match workspace_from_header(&headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    document_put(state, workspace_id, pk, payload).await
}

#[utoipa::path(
    patch,
    path = "/workspaces/{workspace_id}/documents/{*pk}",
    request_body = AnyJson,
    params(
        ("workspace_id" = String, Path, description = "ID do workspace"),
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 200, body = Document, description = "Documento atualizado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn patch_document(
    State(state): State<AppState>,
    Path((workspace_id, pk)): Path<(String, String)>,
    Json(payload): Json<JsonValue>,
) -> Response {
    document_patch(state, workspace_id, pk, payload).await
}

#[utoipa::path(
    patch,
    path = "/documents/{*pk}",
    request_body = AnyJson,
    params(
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 200, body = Document, description = "Documento atualizado"),
        (status = 400, description = "Workspace nao informado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn patch_document_with_header(
    State(state): State<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<JsonValue>,
) -> Response {
    let workspace_id = match workspace_from_header(&headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    document_patch(state, workspace_id, pk, payload).await
}

#[utoipa::path(
    delete,
    path = "/workspaces/{workspace_id}/documents/{*pk}",
    params(
        ("workspace_id" = String, Path, description = "ID do workspace"),
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 204, description = "Removido"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn delete_document(
    State(state): State<AppState>,
    Path((workspace_id, pk)): Path<(String, String)>,
) -> Response {
    document_delete(state, workspace_id, pk).await
}

#[utoipa::path(
    delete,
    path = "/documents/{*pk}",
    params(
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 204, description = "Removido"),
        (status = 400, description = "Workspace nao informado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn delete_document_with_header(
    State(state): State<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
) -> Response {
    let workspace_id = match workspace_from_header(&headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    document_delete(state, workspace_id, pk).await
}

async fn document_create(
    state: AppState,
    workspace_id: String,
    pk: String,
    mut payload: JsonValue,
) -> Response {
    let pk = normalize_pk(&pk);
    if let Some(resp) = ensure_workspace(&state, &workspace_id).await {
        return resp;
    }

    if let JsonValue::Object(map) = &mut payload {
        map.remove("id");
    }

    match state
        .store
        .create_document(&workspace_id, &pk, &payload)
        .await
    {
        Ok(document) => (StatusCode::CREATED, Json(document)).into_response(),
        Err(message) if message == "Document already exists" => {
            json_error(StatusCode::CONFLICT, &message)
        }
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

async fn document_get(state: AppState, workspace_id: String, pk: String) -> Response {
    let pk = normalize_pk(&pk);
    if let Some(resp) = ensure_workspace(&state, &workspace_id).await {
        return resp;
    }

    match state.store.fetch_document(&workspace_id, &pk).await {
        Ok(Some(doc)) => (StatusCode::OK, Json(doc)).into_response(),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "Document not found"),
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

async fn document_put(
    state: AppState,
    workspace_id: String,
    pk: String,
    mut payload: JsonValue,
) -> Response {
    let pk = normalize_pk(&pk);
    if let Some(resp) = ensure_workspace(&state, &workspace_id).await {
        return resp;
    }

    if let JsonValue::Object(map) = &mut payload {
        map.remove("id");
    }

    match state
        .store
        .upsert_document(&workspace_id, &pk, &payload)
        .await
    {
        Ok((created, document)) => {
            let status = if created {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            };
            (status, Json(document)).into_response()
        }
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

async fn document_patch(
    state: AppState,
    workspace_id: String,
    pk: String,
    mut payload: JsonValue,
) -> Response {
    let pk = normalize_pk(&pk);
    if let Some(resp) = ensure_workspace(&state, &workspace_id).await {
        return resp;
    }

    if let JsonValue::Object(map) = &mut payload {
        map.remove("id");
    } else {
        return json_error(StatusCode::BAD_REQUEST, "Body must be a JSON object");
    }

    let mut existing = match state.store.fetch_document(&workspace_id, &pk).await {
        Ok(Some(doc)) => doc,
        Ok(None) => return json_error(StatusCode::NOT_FOUND, "Document not found"),
        Err(message) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    };

    let existing_map = match existing.data.as_object_mut() {
        Some(map) => map,
        None => return json_error(StatusCode::BAD_REQUEST, "Stored value is not an object"),
    };

    let patch_map = payload.as_object().expect("payload checked as object");
    for (key, value) in patch_map {
        if key == "id" {
            continue;
        }
        existing_map.insert(key.clone(), value.clone());
    }

    match state
        .store
        .update_document_data(&workspace_id, &pk, &existing.data)
        .await
    {
        Ok(Some(document)) => (StatusCode::OK, Json(document)).into_response(),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "Document not found"),
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

async fn document_delete(state: AppState, workspace_id: String, pk: String) -> Response {
    let pk = normalize_pk(&pk);
    if let Some(resp) = ensure_workspace(&state, &workspace_id).await {
        return resp;
    }

    match state.store.delete_document(&workspace_id, &pk).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => json_error(StatusCode::NOT_FOUND, "Document not found"),
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

async fn ensure_workspace(state: &AppState, workspace_id: &str) -> Option<Response> {
    match state.store.workspace_exists(workspace_id).await {
        Ok(true) => None,
        Ok(false) => Some(json_error(StatusCode::NOT_FOUND, "Workspace not found")),
        Err(message) => Some(json_error(StatusCode::INTERNAL_SERVER_ERROR, &message)),
    }
}

fn workspace_from_header(headers: &HeaderMap) -> Result<String, Response> {
    let header = headers
        .get("x-workspace-id")
        .ok_or_else(|| json_error(StatusCode::BAD_REQUEST, "x-workspace-id header required"))?;
    let value = header
        .to_str()
        .map_err(|_| json_error(StatusCode::BAD_REQUEST, "Invalid x-workspace-id header"))?;
    if value.trim().is_empty() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "x-workspace-id header required",
        ));
    }
    Ok(value.to_string())
}

fn normalize_pk(pk: &str) -> String {
    let trimmed = pk.trim();
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

fn json_error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}
