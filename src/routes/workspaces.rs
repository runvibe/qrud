use axum::extract::{Extension, Path};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::models::{WorkspaceInput, WorkspaceOutput, WorkspacePatch};
use crate::routes::common::{is_dash_case, json_error, workspace_to_output};
use crate::services::AppState;

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
