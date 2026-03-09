use axum::Json;
use axum::extract::{Extension, Path, Query};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Value as JsonValue, json};

use crate::models::{AnyJson, DocumentOutput};
use crate::routes::common::{
    ListParams, document_to_output, ensure_workspace, json_error, parse_document_selector,
    validate_contract_payload, validate_contract_route, workspace_from_header,
};
use crate::services::AppState;

#[utoipa::path(
    post,
    path = "/workspaces/{workspace}/{*pk}",
    request_body = AnyJson,
    params(
        ("workspace" = String, Path, description = "Workspace name"),
        ("pk" = String, Path, description = "Document path key")
    ),
    responses(
        (status = 201, body = DocumentOutput, description = "Document created"),
        (status = 404, description = "Workspace not found"),
        (status = 409, description = "Document already exists")
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
        ("workspace" = String, Path, description = "Workspace name"),
        ("pk" = String, Path, description = "Document path key")
    ),
    responses(
        (status = 200, body = [DocumentOutput], description = "Document found"),
        (status = 404, description = "Not found")
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
        ("workspace" = String, Path, description = "Workspace name"),
        ("pk" = String, Path, description = "Document path key")
    ),
    responses(
        (status = 200, body = DocumentOutput, description = "Document updated"),
        (status = 201, body = DocumentOutput, description = "Document created"),
        (status = 404, description = "Workspace not found")
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
        ("workspace" = String, Path, description = "Workspace name"),
        ("pk" = String, Path, description = "Document path key")
    ),
    responses(
        (status = 200, body = DocumentOutput, description = "Document updated"),
        (status = 404, description = "Not found")
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
        ("workspace" = String, Path, description = "Workspace name"),
        ("pk" = String, Path, description = "Document path key")
    ),
    responses(
        (status = 204, description = "Removed"),
        (status = 404, description = "Not found")
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
        ("pk" = String, Path, description = "Document path key")
    ),
    responses(
        (status = 201, body = DocumentOutput, description = "Document created"),
        (status = 400, description = "Workspace not provided"),
        (status = 404, description = "Workspace not found"),
        (status = 409, description = "Document already exists")
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
        ("pk" = String, Path, description = "Document path key")
    ),
    responses(
        (status = 200, body = DocumentOutput, description = "Document found"),
        (status = 400, description = "Workspace not provided"),
        (status = 404, description = "Not found")
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
        ("pk" = String, Path, description = "Document path key")
    ),
    responses(
        (status = 200, body = DocumentOutput, description = "Document updated"),
        (status = 201, body = DocumentOutput, description = "Document created"),
        (status = 400, description = "Workspace not provided"),
        (status = 404, description = "Workspace not found")
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
        ("pk" = String, Path, description = "Document path key")
    ),
    responses(
        (status = 200, body = DocumentOutput, description = "Document updated"),
        (status = 400, description = "Workspace not provided"),
        (status = 404, description = "Not found")
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
        ("pk" = String, Path, description = "Document path key")
    ),
    responses(
        (status = 204, description = "Removed"),
        (status = 400, description = "Workspace not provided"),
        (status = 404, description = "Not found")
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
        (status = 200, description = "Integration information")
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
        Ok(document) => (StatusCode::CREATED, Json(document_to_output(document))).into_response(),
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
        return match state
            .store
            .fetch_document_by_id(&workspace_data.id, id)
            .await
        {
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
        state
            .store
            .fetch_document_by_id(&workspace_data.id, id)
            .await
    } else {
        state
            .store
            .fetch_document(&workspace_data.id, &selector.pk)
            .await
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
