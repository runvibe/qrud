use axum::extract::{Extension, Path};
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
    Extension(state): Extension<AppState>,
    Json(payload): Json<WorkspaceInput>,
) -> Response {
    let name = payload.name.trim();
    if name.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "Workspace name is required");
    }
    if !is_dash_case(name) {
        return json_error(StatusCode::BAD_REQUEST, "Workspace name must be dash-case");
    }
    match state
        .store
        .create_workspace(name, payload.description.as_deref())
        .await
    {
        Ok(workspace) => (StatusCode::CREATED, Json(workspace)).into_response(),
        Err(message) if message == "Workspace already exists" => {
            json_error(StatusCode::CONFLICT, &message)
        }
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
pub(crate) async fn list_workspaces(Extension(state): Extension<AppState>) -> Response {
    match state.store.list_workspaces().await {
        Ok(workspaces) => (StatusCode::OK, Json(workspaces)).into_response(),
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

#[utoipa::path(
    post,
    path = "/w",
    request_body = WorkspaceInput,
    responses(
        (status = 201, body = Workspace, description = "Workspace criado")
    )
)]
pub(crate) async fn create_workspace_short(
    Extension(state): Extension<AppState>,
    Json(payload): Json<WorkspaceInput>,
) -> Response {
    create_workspace(Extension(state), Json(payload)).await
}

#[utoipa::path(
    get,
    path = "/w",
    responses(
        (status = 200, body = [Workspace], description = "Lista de workspaces")
    )
)]
pub(crate) async fn list_workspaces_short(Extension(state): Extension<AppState>) -> Response {
    list_workspaces(Extension(state)).await
}

#[utoipa::path(
    get,
    path = "/workspaces/{workspace}",
    params(
        ("workspace" = String, Path, description = "Nome do workspace")
    ),
    responses(
        (status = 200, body = Workspace, description = "Workspace encontrado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn get_workspace(
    Extension(state): Extension<AppState>,
    Path(workspace): Path<String>,
) -> Response {
    if !is_dash_case(workspace.trim()) {
        return json_error(StatusCode::BAD_REQUEST, "Workspace name must be dash-case");
    }
    match state.store.fetch_workspace_by_name(&workspace).await {
        Ok(Some(workspace)) => (StatusCode::OK, Json(workspace)).into_response(),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "Workspace not found"),
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

#[utoipa::path(
    get,
    path = "/w/{workspace}",
    params(
        ("workspace" = String, Path, description = "Nome do workspace")
    ),
    responses(
        (status = 200, body = Workspace, description = "Workspace encontrado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn get_workspace_short(
    Extension(state): Extension<AppState>,
    Path(workspace): Path<String>,
) -> Response {
    get_workspace(Extension(state), Path(workspace)).await
}

#[utoipa::path(
    put,
    path = "/workspaces/{workspace}",
    request_body = WorkspaceInput,
    params(
        ("workspace" = String, Path, description = "Nome do workspace")
    ),
    responses(
        (status = 200, body = Workspace, description = "Workspace atualizado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn put_workspace(
    Extension(state): Extension<AppState>,
    Path(workspace): Path<String>,
    Json(payload): Json<WorkspaceInput>,
) -> Response {
    if !is_dash_case(workspace.trim()) {
        return json_error(StatusCode::BAD_REQUEST, "Workspace name must be dash-case");
    }
    let name = payload.name.trim();
    if name.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "Workspace name is required");
    }
    if !is_dash_case(name) {
        return json_error(StatusCode::BAD_REQUEST, "Workspace name must be dash-case");
    }
    match state
        .store
        .update_workspace(&workspace, name, payload.description.as_deref())
        .await
    {
        Ok(Some(workspace)) => (StatusCode::OK, Json(workspace)).into_response(),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "Workspace not found"),
        Err(message) if message == "Workspace already exists" => {
            json_error(StatusCode::CONFLICT, &message)
        }
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

#[utoipa::path(
    put,
    path = "/w/{workspace}",
    request_body = WorkspaceInput,
    params(
        ("workspace" = String, Path, description = "Nome do workspace")
    ),
    responses(
        (status = 200, body = Workspace, description = "Workspace atualizado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn put_workspace_short(
    Extension(state): Extension<AppState>,
    Path(workspace): Path<String>,
    Json(payload): Json<WorkspaceInput>,
) -> Response {
    put_workspace(Extension(state), Path(workspace), Json(payload)).await
}

#[utoipa::path(
    patch,
    path = "/workspaces/{workspace}",
    request_body = WorkspacePatch,
    params(
        ("workspace" = String, Path, description = "Nome do workspace")
    ),
    responses(
        (status = 200, body = Workspace, description = "Workspace atualizado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn patch_workspace(
    Extension(state): Extension<AppState>,
    Path(workspace): Path<String>,
    Json(payload): Json<WorkspacePatch>,
) -> Response {
    if !is_dash_case(workspace.trim()) {
        return json_error(StatusCode::BAD_REQUEST, "Workspace name must be dash-case");
    }
    let mut workspace_data = match state.store.fetch_workspace_by_name(&workspace).await {
        Ok(Some(workspace)) => workspace,
        Ok(None) => return json_error(StatusCode::NOT_FOUND, "Workspace not found"),
        Err(message) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    };

    let mut changed = false;
    if let Some(name) = payload.name {
        let name = name.trim().to_string();
        if name.is_empty() {
            return json_error(StatusCode::BAD_REQUEST, "Workspace name is required");
        }
        if !is_dash_case(&name) {
            return json_error(StatusCode::BAD_REQUEST, "Workspace name must be dash-case");
        }
        workspace_data.name = name;
        changed = true;
    }
    if let Some(description) = payload.description {
        workspace_data.description = Some(description);
        changed = true;
    }

    if !changed {
        return json_error(StatusCode::BAD_REQUEST, "No fields to update");
    }

    match state
        .store
        .update_workspace(
            &workspace,
            &workspace_data.name,
            workspace_data.description.as_deref(),
        )
        .await
    {
        Ok(Some(workspace)) => (StatusCode::OK, Json(workspace)).into_response(),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "Workspace not found"),
        Err(message) if message == "Workspace already exists" => {
            json_error(StatusCode::CONFLICT, &message)
        }
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

#[utoipa::path(
    patch,
    path = "/w/{workspace}",
    request_body = WorkspacePatch,
    params(
        ("workspace" = String, Path, description = "Nome do workspace")
    ),
    responses(
        (status = 200, body = Workspace, description = "Workspace atualizado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn patch_workspace_short(
    Extension(state): Extension<AppState>,
    Path(workspace): Path<String>,
    Json(payload): Json<WorkspacePatch>,
) -> Response {
    patch_workspace(Extension(state), Path(workspace), Json(payload)).await
}

#[utoipa::path(
    delete,
    path = "/workspaces/{workspace}",
    params(
        ("workspace" = String, Path, description = "Nome do workspace")
    ),
    responses(
        (status = 204, description = "Removido"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn delete_workspace(
    Extension(state): Extension<AppState>,
    Path(workspace): Path<String>,
) -> Response {
    if !is_dash_case(workspace.trim()) {
        return json_error(StatusCode::BAD_REQUEST, "Workspace name must be dash-case");
    }
    match state.store.delete_workspace(&workspace).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => json_error(StatusCode::NOT_FOUND, "Workspace not found"),
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

#[utoipa::path(
    delete,
    path = "/w/{workspace}",
    params(
        ("workspace" = String, Path, description = "Nome do workspace")
    ),
    responses(
        (status = 204, description = "Removido"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn delete_workspace_short(
    Extension(state): Extension<AppState>,
    Path(workspace): Path<String>,
) -> Response {
    delete_workspace(Extension(state), Path(workspace)).await
}

#[utoipa::path(
    post,
    path = "/workspaces/{workspace}/documents/{*pk}",
    request_body = AnyJson,
    params(
        ("workspace" = String, Path, description = "Nome do workspace"),
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 201, body = Document, description = "Documento criado"),
        (status = 404, description = "Workspace nao encontrado"),
        (status = 409, description = "Documento ja existe")
    )
)]
pub(crate) async fn create_document(
    Extension(state): Extension<AppState>,
    Path((workspace, pk)): Path<(String, String)>,
    Json(payload): Json<JsonValue>,
) -> Response {
    document_create(state, workspace, pk, payload).await
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
    Extension(state): Extension<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<JsonValue>,
) -> Response {
    let workspace = match workspace_from_header(&headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    document_create(state, workspace, pk, payload).await
}

#[utoipa::path(
    post,
    path = "/{*pk}",
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
pub(crate) async fn create_document_root(
    Extension(state): Extension<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<JsonValue>,
) -> Response {
    create_document_with_header(Extension(state), Path(pk), headers, Json(payload)).await
}

#[utoipa::path(
    post,
    path = "/d/{*pk}",
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
pub(crate) async fn create_document_short(
    Extension(state): Extension<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<JsonValue>,
) -> Response {
    create_document_with_header(Extension(state), Path(pk), headers, Json(payload)).await
}

#[utoipa::path(
    get,
    path = "/workspaces/{workspace}/documents/{*pk}",
    params(
        ("workspace" = String, Path, description = "Nome do workspace"),
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 200, body = Document, description = "Documento encontrado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn get_document(
    Extension(state): Extension<AppState>,
    Path((workspace, pk)): Path<(String, String)>,
) -> Response {
    document_get(state, workspace, pk).await
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
    Extension(state): Extension<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
) -> Response {
    let workspace = match workspace_from_header(&headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    document_get(state, workspace, pk).await
}

#[utoipa::path(
    get,
    path = "/{*pk}",
    params(
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 200, body = Document, description = "Documento encontrado"),
        (status = 400, description = "Workspace nao informado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn get_document_root(
    Extension(state): Extension<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
) -> Response {
    get_document_with_header(Extension(state), Path(pk), headers).await
}

#[utoipa::path(
    get,
    path = "/d/{*pk}",
    params(
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 200, body = Document, description = "Documento encontrado"),
        (status = 400, description = "Workspace nao informado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn get_document_short(
    Extension(state): Extension<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
) -> Response {
    get_document_with_header(Extension(state), Path(pk), headers).await
}

#[utoipa::path(
    put,
    path = "/workspaces/{workspace}/documents/{*pk}",
    request_body = AnyJson,
    params(
        ("workspace" = String, Path, description = "Nome do workspace"),
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 200, body = Document, description = "Documento atualizado"),
        (status = 201, body = Document, description = "Documento criado"),
        (status = 404, description = "Workspace nao encontrado")
    )
)]
pub(crate) async fn put_document(
    Extension(state): Extension<AppState>,
    Path((workspace, pk)): Path<(String, String)>,
    Json(payload): Json<JsonValue>,
) -> Response {
    document_put(state, workspace, pk, payload).await
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
    Extension(state): Extension<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<JsonValue>,
) -> Response {
    let workspace = match workspace_from_header(&headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    document_put(state, workspace, pk, payload).await
}

#[utoipa::path(
    put,
    path = "/{*pk}",
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
pub(crate) async fn put_document_root(
    Extension(state): Extension<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<JsonValue>,
) -> Response {
    put_document_with_header(Extension(state), Path(pk), headers, Json(payload)).await
}

#[utoipa::path(
    put,
    path = "/d/{*pk}",
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
pub(crate) async fn put_document_short(
    Extension(state): Extension<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<JsonValue>,
) -> Response {
    put_document_with_header(Extension(state), Path(pk), headers, Json(payload)).await
}

#[utoipa::path(
    patch,
    path = "/workspaces/{workspace}/documents/{*pk}",
    request_body = AnyJson,
    params(
        ("workspace" = String, Path, description = "Nome do workspace"),
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 200, body = Document, description = "Documento atualizado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn patch_document(
    Extension(state): Extension<AppState>,
    Path((workspace, pk)): Path<(String, String)>,
    Json(payload): Json<JsonValue>,
) -> Response {
    document_patch(state, workspace, pk, payload).await
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
    Extension(state): Extension<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<JsonValue>,
) -> Response {
    let workspace = match workspace_from_header(&headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    document_patch(state, workspace, pk, payload).await
}

#[utoipa::path(
    patch,
    path = "/{*pk}",
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
pub(crate) async fn patch_document_root(
    Extension(state): Extension<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<JsonValue>,
) -> Response {
    patch_document_with_header(Extension(state), Path(pk), headers, Json(payload)).await
}

#[utoipa::path(
    patch,
    path = "/d/{*pk}",
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
pub(crate) async fn patch_document_short(
    Extension(state): Extension<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<JsonValue>,
) -> Response {
    patch_document_with_header(Extension(state), Path(pk), headers, Json(payload)).await
}

#[utoipa::path(
    delete,
    path = "/workspaces/{workspace}/documents/{*pk}",
    params(
        ("workspace" = String, Path, description = "Nome do workspace"),
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 204, description = "Removido"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn delete_document(
    Extension(state): Extension<AppState>,
    Path((workspace, pk)): Path<(String, String)>,
) -> Response {
    document_delete(state, workspace, pk).await
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
    Extension(state): Extension<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
) -> Response {
    let workspace = match workspace_from_header(&headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    document_delete(state, workspace, pk).await
}

#[utoipa::path(
    delete,
    path = "/{*pk}",
    params(
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 204, description = "Removido"),
        (status = 400, description = "Workspace nao informado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn delete_document_root(
    Extension(state): Extension<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
) -> Response {
    delete_document_with_header(Extension(state), Path(pk), headers).await
}

#[utoipa::path(
    delete,
    path = "/d/{*pk}",
    params(
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 204, description = "Removido"),
        (status = 400, description = "Workspace nao informado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn delete_document_short(
    Extension(state): Extension<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
) -> Response {
    delete_document_with_header(Extension(state), Path(pk), headers).await
}

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "OK")
    )
)]
pub(crate) async fn health() -> Response {
    (StatusCode::OK, "OK").into_response()
}

#[utoipa::path(
    get,
    path = "/info",
    responses(
        (status = 200, description = "Informacoes de integracao")
    )
)]
pub(crate) async fn info(Extension(state): Extension<AppState>) -> Response {
    let payload = json!({
        "database": {
            "backend": state.store.backend_name()
        }
    });
    (StatusCode::OK, Json(payload)).into_response()
}

async fn document_create(
    state: AppState,
    workspace: String,
    pk: String,
    mut payload: JsonValue,
) -> Response {
    let pk = normalize_pk(&pk);
    let workspace_data = match ensure_workspace(&state, &workspace).await {
        Ok(workspace) => workspace,
        Err(resp) => return resp,
    };

    if let JsonValue::Object(map) = &mut payload {
        map.remove("id");
    }

    match state
        .store
        .create_document(&workspace_data.id, &pk, &payload)
        .await
    {
        Ok(document) => (StatusCode::CREATED, Json(document)).into_response(),
        Err(message) if message == "Document already exists" => {
            json_error(StatusCode::CONFLICT, &message)
        }
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

async fn document_get(state: AppState, workspace: String, pk: String) -> Response {
    let pk = normalize_pk(&pk);
    let workspace_data = match ensure_workspace(&state, &workspace).await {
        Ok(workspace) => workspace,
        Err(resp) => return resp,
    };

    match state.store.fetch_document(&workspace_data.id, &pk).await {
        Ok(Some(doc)) => (StatusCode::OK, Json(doc)).into_response(),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "Document not found"),
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

async fn document_put(
    state: AppState,
    workspace: String,
    pk: String,
    mut payload: JsonValue,
) -> Response {
    let pk = normalize_pk(&pk);
    let workspace_data = match ensure_workspace(&state, &workspace).await {
        Ok(workspace) => workspace,
        Err(resp) => return resp,
    };

    if let JsonValue::Object(map) = &mut payload {
        map.remove("id");
    }

    match state
        .store
        .upsert_document(&workspace_data.id, &pk, &payload)
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
    workspace: String,
    pk: String,
    mut payload: JsonValue,
) -> Response {
    let pk = normalize_pk(&pk);
    let workspace_data = match ensure_workspace(&state, &workspace).await {
        Ok(workspace) => workspace,
        Err(resp) => return resp,
    };

    if let JsonValue::Object(map) = &mut payload {
        map.remove("id");
    } else {
        return json_error(StatusCode::BAD_REQUEST, "Body must be a JSON object");
    }

    let mut existing = match state.store.fetch_document(&workspace_data.id, &pk).await {
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
        .update_document_data(&workspace_data.id, &pk, &existing.data)
        .await
    {
        Ok(Some(document)) => (StatusCode::OK, Json(document)).into_response(),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "Document not found"),
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

async fn document_delete(state: AppState, workspace: String, pk: String) -> Response {
    let pk = normalize_pk(&pk);
    let workspace_data = match ensure_workspace(&state, &workspace).await {
        Ok(workspace) => workspace,
        Err(resp) => return resp,
    };

    match state.store.delete_document(&workspace_data.id, &pk).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => json_error(StatusCode::NOT_FOUND, "Document not found"),
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

async fn ensure_workspace(state: &AppState, workspace: &str) -> Result<Workspace, Response> {
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

fn workspace_from_header(headers: &HeaderMap) -> Result<String, Response> {
    let header = headers
        .get("x-workspace-id")
        .ok_or_else(|| json_error(StatusCode::BAD_REQUEST, "x-workspace-id header required"))?;
    let value = header
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

fn normalize_pk(pk: &str) -> String {
    let trimmed = pk.trim();
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

fn is_dash_case(value: &str) -> bool {
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

fn is_dash_char(ch: char, allow_dash: bool) -> bool {
    match ch {
        'a'..='z' | '0'..='9' => true,
        '-' => allow_dash,
        _ => false,
    }
}

fn json_error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}
