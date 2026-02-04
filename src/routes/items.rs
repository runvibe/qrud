use axum::extract::{Extension, Path, Query};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::models::{
    AnyJson, Document, DocumentOutput, Workspace, WorkspaceInput, WorkspaceOutput, WorkspacePatch,
};
use crate::services::{AppState, DEFAULT_WORKSPACE_NAME};

#[utoipa::path(
    post,
    path = "/workspaces",
    request_body = WorkspaceInput,
    responses(
        (status = 201, body = WorkspaceOutput, description = "Workspace criado")
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
        Ok(workspace) => (StatusCode::CREATED, Json(workspace_to_output(workspace))).into_response(),
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
        (status = 200, body = [WorkspaceOutput], description = "Lista de workspaces")
    )
)]
pub(crate) async fn list_workspaces(Extension(state): Extension<AppState>) -> Response {
    match state.store.list_workspaces().await {
        Ok(workspaces) => {
            let output = workspaces
                .into_iter()
                .map(workspace_to_output)
                .collect::<Vec<_>>();
            (StatusCode::OK, Json(output)).into_response()
        }
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

#[utoipa::path(
    get,
    path = "/workspaces/{workspace}",
    params(
        ("workspace" = String, Path, description = "Nome do workspace")
    ),
    responses(
        (status = 200, body = WorkspaceOutput, description = "Workspace encontrado"),
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
        Ok(Some(workspace)) => (StatusCode::OK, Json(workspace_to_output(workspace))).into_response(),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "Workspace not found"),
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

#[utoipa::path(
    put,
    path = "/workspaces/{workspace}",
    request_body = WorkspaceInput,
    params(
        ("workspace" = String, Path, description = "Nome do workspace")
    ),
    responses(
        (status = 200, body = WorkspaceOutput, description = "Workspace atualizado"),
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
        Ok(Some(workspace)) => (StatusCode::OK, Json(workspace_to_output(workspace))).into_response(),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "Workspace not found"),
        Err(message) if message == "Workspace already exists" => {
            json_error(StatusCode::CONFLICT, &message)
        }
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

#[utoipa::path(
    patch,
    path = "/workspaces/{workspace}",
    request_body = WorkspacePatch,
    params(
        ("workspace" = String, Path, description = "Nome do workspace")
    ),
    responses(
        (status = 200, body = WorkspaceOutput, description = "Workspace atualizado"),
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
        Ok(Some(workspace)) => (StatusCode::OK, Json(workspace_to_output(workspace))).into_response(),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "Workspace not found"),
        Err(message) if message == "Workspace already exists" => {
            json_error(StatusCode::CONFLICT, &message)
        }
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
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
    post,
    path = "/workspaces/{workspace}/{*pk}",
    request_body = AnyJson,
    params(
        ("workspace" = String, Path, description = "Nome do workspace"),
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 201, body = DocumentOutput, description = "Documento criado"),
        (status = 404, description = "Workspace nao encontrado"),
        (status = 409, description = "Documento ja existe")
    )
)]
pub(crate) async fn create_document_workspace(
    Extension(state): Extension<AppState>,
    Path((workspace, pk)): Path<(String, String)>,
    Json(payload): Json<JsonValue>,
) -> Response {
    document_create(state, workspace, pk, payload).await
}

#[utoipa::path(
    get,
    path = "/workspaces/{workspace}/{*pk}",
    params(
        ("workspace" = String, Path, description = "Nome do workspace"),
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 200, body = [DocumentOutput], description = "Documento encontrado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn get_document_workspace(
    Extension(state): Extension<AppState>,
    Path((workspace, pk)): Path<(String, String)>,
    Query(params): Query<ListParams>,
) -> Response {
    document_get(state, workspace, pk, params).await
}

#[utoipa::path(
    put,
    path = "/workspaces/{workspace}/{*pk}",
    request_body = AnyJson,
    params(
        ("workspace" = String, Path, description = "Nome do workspace"),
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 200, body = DocumentOutput, description = "Documento atualizado"),
        (status = 201, body = DocumentOutput, description = "Documento criado"),
        (status = 404, description = "Workspace nao encontrado")
    )
)]
pub(crate) async fn put_document_workspace(
    Extension(state): Extension<AppState>,
    Path((workspace, pk)): Path<(String, String)>,
    Json(payload): Json<JsonValue>,
) -> Response {
    document_put(state, workspace, pk, payload).await
}

#[utoipa::path(
    patch,
    path = "/workspaces/{workspace}/{*pk}",
    request_body = AnyJson,
    params(
        ("workspace" = String, Path, description = "Nome do workspace"),
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 200, body = DocumentOutput, description = "Documento atualizado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn patch_document_workspace(
    Extension(state): Extension<AppState>,
    Path((workspace, pk)): Path<(String, String)>,
    Json(payload): Json<JsonValue>,
) -> Response {
    document_patch(state, workspace, pk, payload).await
}

#[utoipa::path(
    delete,
    path = "/workspaces/{workspace}/{*pk}",
    params(
        ("workspace" = String, Path, description = "Nome do workspace"),
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 204, description = "Removido"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn delete_document_workspace(
    Extension(state): Extension<AppState>,
    Path((workspace, pk)): Path<(String, String)>,
) -> Response {
    document_delete(state, workspace, pk).await
}

#[utoipa::path(
    post,
    path = "/{*pk}",
    request_body = AnyJson,
    params(
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 201, body = DocumentOutput, description = "Documento criado"),
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
    let workspace = match workspace_from_header(&headers, state.use_default_workspace) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    document_create(state, workspace, pk, payload).await
}

#[utoipa::path(
    get,
    path = "/{*pk}",
    params(
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 200, body = DocumentOutput, description = "Documento encontrado"),
        (status = 400, description = "Workspace nao informado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn get_document_root(
    Extension(state): Extension<AppState>,
    Path(pk): Path<String>,
    headers: HeaderMap,
    Query(params): Query<ListParams>,
) -> Response {
    let workspace = match workspace_from_header(&headers, state.use_default_workspace) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    document_get(state, workspace, pk, params).await
}

#[utoipa::path(
    put,
    path = "/{*pk}",
    request_body = AnyJson,
    params(
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 200, body = DocumentOutput, description = "Documento atualizado"),
        (status = 201, body = DocumentOutput, description = "Documento criado"),
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
    let workspace = match workspace_from_header(&headers, state.use_default_workspace) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    document_put(state, workspace, pk, payload).await
}

#[utoipa::path(
    patch,
    path = "/{*pk}",
    request_body = AnyJson,
    params(
        ("pk" = String, Path, description = "Path key do documento")
    ),
    responses(
        (status = 200, body = DocumentOutput, description = "Documento atualizado"),
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
    let workspace = match workspace_from_header(&headers, state.use_default_workspace) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    document_patch(state, workspace, pk, payload).await
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
    let workspace = match workspace_from_header(&headers, state.use_default_workspace) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    document_delete(state, workspace, pk).await
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
        "database": state.store.database_info()
    });
    (StatusCode::OK, Json(payload)).into_response()
}

async fn document_create(
    state: AppState,
    workspace: String,
    pk: String,
    mut payload: JsonValue,
) -> Response {
    let selector = match parse_document_selector(&pk) {
        Ok(selector) => selector,
        Err(resp) => return resp,
    };
    if selector.id.is_some() {
        return json_error(StatusCode::BAD_REQUEST, "Document id not allowed for POST");
    }
    if let Err(resp) = validate_contract_route(&state, "post", &selector) {
        return resp;
    }
    let workspace_data = match ensure_workspace(&state, &workspace).await {
        Ok(workspace) => workspace,
        Err(resp) => return resp,
    };

    if let JsonValue::Object(map) = &mut payload {
        map.remove("id");
    }
    if let Err(resp) = validate_contract_payload(&state, "post", &selector, &payload) {
        return resp;
    }

    match state
        .store
        .create_document(&workspace_data.id, &selector.pk, &payload)
        .await
    {
        Ok(document) => {
            (StatusCode::CREATED, Json(document_to_output(document))).into_response()
        }
        Err(message) if message == "Document already exists" => {
            json_error(StatusCode::CONFLICT, &message)
        }
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

async fn document_get(
    state: AppState,
    workspace: String,
    pk: String,
    params: ListParams,
) -> Response {
    let selector = match parse_document_selector(&pk) {
        Ok(selector) => selector,
        Err(resp) => return resp,
    };
    if let Err(resp) = validate_contract_route(&state, "get", &selector) {
        return resp;
    }
    let workspace_data = match ensure_workspace(&state, &workspace).await {
        Ok(workspace) => workspace,
        Err(resp) => return resp,
    };

    if let Some(id) = selector.id.as_deref() {
        return match state.store.fetch_document_by_id(&workspace_data.id, id).await {
            Ok(Some(doc)) => (StatusCode::OK, Json(document_to_output(doc))).into_response(),
            Ok(None) => json_error(StatusCode::NOT_FOUND, "Document not found"),
            Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
        };
    }

    let offset = params.offset.unwrap_or(0).max(0);
    let limit = params.limit.filter(|value| *value > 0);
    let term = params
        .term
        .as_ref()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());

    let order_value = params.order.as_deref().unwrap_or("desc");
    let order_value = order_value.to_ascii_lowercase();
    let order_str = match order_value.as_str() {
        "asc" => "asc",
        "desc" => "desc",
        _ => return json_error(StatusCode::BAD_REQUEST, "Invalid order"),
    };
    let order_desc = order_str == "desc";

    let by_value = params.by.as_deref().unwrap_or("created_at");
    let by_value = by_value.to_ascii_lowercase();
    let by_str = match by_value.as_str() {
        "created_at" => "created_at",
        "updated_at" | "update_at" => "updated_at",
        _ => return json_error(StatusCode::BAD_REQUEST, "Invalid by"),
    };

    match state
        .store
        .fetch_documents_by_pk(
            &workspace_data.id,
            &selector.pk,
            term.as_deref(),
            limit,
            offset,
            order_desc,
            by_str,
        )
        .await
    {
        Ok(docs) => {
            let total = match state
                .store
                .fetch_meta_pk_total(&workspace_data.id, &selector.pk)
                .await
            {
                Ok(total) => total.unwrap_or(0),
                Err(message) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
            };
            let items = docs.into_iter().map(document_to_output).collect::<Vec<_>>();
            let limit_used = limit.unwrap_or(items.len() as i64);
            let payload = json!({
                "items": items,
                "total": total,
                "limit": limit_used,
                "offset": offset,
                "order": order_str,
                "by": by_str
            });
            (StatusCode::OK, Json(payload)).into_response()
        }
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

async fn document_put(
    state: AppState,
    workspace: String,
    pk: String,
    mut payload: JsonValue,
) -> Response {
    let selector = match parse_document_selector(&pk) {
        Ok(selector) => selector,
        Err(resp) => return resp,
    };
    if let Err(resp) = validate_contract_route(&state, "put", &selector) {
        return resp;
    }
    let workspace_data = match ensure_workspace(&state, &workspace).await {
        Ok(workspace) => workspace,
        Err(resp) => return resp,
    };

    if let JsonValue::Object(map) = &mut payload {
        map.remove("id");
    }
    if let Err(resp) = validate_contract_payload(&state, "put", &selector, &payload) {
        return resp;
    }

    let result = if let Some(id) = selector.id.as_deref() {
        state
            .store
            .upsert_document_by_id(&workspace_data.id, id, &selector.pk, &payload)
            .await
    } else {
        state
            .store
            .upsert_document(&workspace_data.id, &selector.pk, &payload)
            .await
    };

    match result {
        Ok((created, document)) => {
            let status = if created {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            };
            (status, Json(document_to_output(document))).into_response()
        }
        Err(message) if message == "Document already exists" => {
            json_error(StatusCode::CONFLICT, &message)
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
    let selector = match parse_document_selector(&pk) {
        Ok(selector) => selector,
        Err(resp) => return resp,
    };
    if let Err(resp) = validate_contract_route(&state, "patch", &selector) {
        return resp;
    }
    let workspace_data = match ensure_workspace(&state, &workspace).await {
        Ok(workspace) => workspace,
        Err(resp) => return resp,
    };

    if let JsonValue::Object(map) = &mut payload {
        map.remove("id");
    } else {
        return json_error(StatusCode::BAD_REQUEST, "Body must be a JSON object");
    }
    if let Err(resp) = validate_contract_payload(&state, "patch", &selector, &payload) {
        return resp;
    }

    let existing_result = if let Some(id) = selector.id.as_deref() {
        state.store.fetch_document_by_id(&workspace_data.id, id).await
    } else {
        state.store.fetch_document(&workspace_data.id, &selector.pk).await
    };

    let mut existing = match existing_result {
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

    let update_result = if let Some(id) = selector.id.as_deref() {
        state
            .store
            .update_document_data_by_id(&workspace_data.id, id, &existing.data)
            .await
    } else {
        state
            .store
            .update_document_data(&workspace_data.id, &selector.pk, &existing.data)
            .await
    };

    match update_result {
        Ok(Some(document)) => (StatusCode::OK, Json(document_to_output(document))).into_response(),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "Document not found"),
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

async fn document_delete(state: AppState, workspace: String, pk: String) -> Response {
    let selector = match parse_document_selector(&pk) {
        Ok(selector) => selector,
        Err(resp) => return resp,
    };
    if let Err(resp) = validate_contract_route(&state, "delete", &selector) {
        return resp;
    }
    let workspace_data = match ensure_workspace(&state, &workspace).await {
        Ok(workspace) => workspace,
        Err(resp) => return resp,
    };

    let result = if let Some(id) = selector.id.as_deref() {
        state
            .store
            .delete_document_by_id(&workspace_data.id, id)
            .await
    } else {
        state
            .store
            .delete_document(&workspace_data.id, &selector.pk)
            .await
    };

    match result {
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

fn workspace_from_header(headers: &HeaderMap, use_default: bool) -> Result<String, Response> {
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

struct DocumentSelector {
    pk: String,
    id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ListParams {
    limit: Option<i64>,
    offset: Option<i64>,
    term: Option<String>,
    order: Option<String>,
    by: Option<String>,
}


fn parse_document_selector(raw: &str) -> Result<DocumentSelector, Response> {
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

fn validate_contract_route(
    state: &AppState,
    method: &str,
    selector: &DocumentSelector,
) -> Result<(), Response> {
    let Some(contract) = state.api_contract.as_ref() else {
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

fn validate_contract_payload(
    state: &AppState,
    method: &str,
    selector: &DocumentSelector,
    payload: &JsonValue,
) -> Result<(), Response> {
    let Some(contract) = state.api_contract.as_ref() else {
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

fn document_to_output(document: Document) -> JsonValue {
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

fn workspace_to_output(workspace: Workspace) -> WorkspaceOutput {
    WorkspaceOutput {
        id: workspace.id,
        name: workspace.name,
        description: workspace.description,
        created_at: format_millis_iso(workspace.created_at),
        updated_at: format_millis_iso(workspace.updated_at),
        deleted_at: workspace.deleted_at.map(format_millis_iso),
    }
}

fn format_millis_iso(millis: i64) -> String {
    let nanos = i128::from(millis) * 1_000_000;
    let datetime = OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .unwrap_or_else(|_| OffsetDateTime::from_unix_timestamp(0).unwrap());
    datetime.format(&Rfc3339).unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
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
