use axum::Json;
use axum::extract::Extension;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::Value as JsonValue;

use crate::models::AnyJson;
use crate::routes::common::json_error;
use crate::services::{ApiContract, AppState};

#[utoipa::path(
    put,
    path = "/openapi/contract",
    request_body = AnyJson,
    responses(
        (status = 204, description = "OpenAPI contract loaded"),
        (status = 400, description = "Invalid OpenAPI contract")
    )
)]
pub(crate) async fn put_contract(
    Extension(state): Extension<AppState>,
    Json(payload): Json<JsonValue>,
) -> Response {
    let contract = match ApiContract::from_value(&payload) {
        Ok(contract) => contract,
        Err(message) => return json_error(StatusCode::BAD_REQUEST, &message),
    };

    state.replace_api_contract(Some(contract));
    StatusCode::NO_CONTENT.into_response()
}

#[utoipa::path(
    delete,
    path = "/openapi/contract",
    responses(
        (status = 204, description = "OpenAPI contract removed")
    )
)]
pub(crate) async fn delete_contract(Extension(state): Extension<AppState>) -> Response {
    state.replace_api_contract(None);
    StatusCode::NO_CONTENT.into_response()
}
