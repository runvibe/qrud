use axum::extract::{Path, RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value as JsonValue};

use crate::models::{AnyJson, ListQuery, DEFAULT_FIELDS};
use crate::services::{lock_store, AppState};

#[utoipa::path(
    get,
    path = "/{collection}",
    params(
        ("collection" = String, Path, description = "Nome da colecao"),
        ("term" = Option<String>, Query, description = "Termo de busca (case-insensitive)"),
        ("filter" = Option<Vec<String>>, Query, description = "Campos consultados (repetir)"),
        ("limit" = Option<usize>, Query, description = "Limite de itens"),
        ("offset" = Option<usize>, Query, description = "Offset de itens")
    ),
    responses(
        (status = 200, body = [AnyJson], description = "Lista de itens")
    )
)]
pub(crate) async fn list_collection(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    RawQuery(raw): RawQuery,
) -> Response {
    let query = match parse_query(raw) {
        Ok(query) => query,
        Err(resp) => return resp,
    };
    list_items(state, &collection, query)
}

#[utoipa::path(
    post,
    path = "/{collection}",
    request_body = AnyJson,
    params(
        ("collection" = String, Path, description = "Nome da colecao")
    ),
    responses(
        (status = 201, body = AnyJson, description = "Item criado")
    )
)]
pub(crate) async fn create_item(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    Json(payload): Json<JsonValue>,
) -> Response {
    let mut obj = match payload {
        JsonValue::Object(map) => map,
        _ => return json_error(StatusCode::BAD_REQUEST, "Body must be a JSON object"),
    };

    let mut store = lock_store(&state);
    let id = match store.next_id_for(&collection) {
        Ok(id) => id,
        Err(message) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    };

    obj.insert("id".to_string(), JsonValue::String(id.to_string()));
    let response_value = JsonValue::Object(obj.clone());
    let data = match serde_json::to_string(&response_value) {
        Ok(data) => data,
        Err(err) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &err.to_string()),
    };

    if let Err(message) = store.insert_item(&collection, id, &data) {
        return json_error(StatusCode::INTERNAL_SERVER_ERROR, &message);
    }

    (StatusCode::CREATED, Json(response_value)).into_response()
}

#[utoipa::path(
    get,
    path = "/{collection}/{id}",
    params(
        ("collection" = String, Path, description = "Nome da colecao"),
        ("id" = i64, Path, description = "ID do item")
    ),
    responses(
        (status = 200, body = AnyJson, description = "Item encontrado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn get_item(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, i64)>,
) -> Response {
    let mut store = lock_store(&state);
    let data = match store.fetch_item_data(&collection, id) {
        Ok(Some(value)) => value,
        Ok(None) => return json_error(StatusCode::NOT_FOUND, "Item not found"),
        Err(message) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    };
    drop(store);

    let json_value = match serde_json::from_str::<JsonValue>(&data) {
        Ok(value) => value,
        Err(err) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &err.to_string()),
    };

    (StatusCode::OK, Json(json_value)).into_response()
}

#[utoipa::path(
    put,
    path = "/{collection}/{id}",
    request_body = AnyJson,
    params(
        ("collection" = String, Path, description = "Nome da colecao"),
        ("id" = i64, Path, description = "ID do item")
    ),
    responses(
        (status = 200, body = AnyJson, description = "Item atualizado"),
        (status = 201, body = AnyJson, description = "Item criado")
    )
)]
pub(crate) async fn put_item(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, i64)>,
    Json(payload): Json<JsonValue>,
) -> Response {
    let mut obj = match payload {
        JsonValue::Object(map) => map,
        _ => return json_error(StatusCode::BAD_REQUEST, "Body must be a JSON object"),
    };

    obj.insert("id".to_string(), JsonValue::String(id.to_string()));
    let response_value = JsonValue::Object(obj.clone());

    let data = match serde_json::to_string(&response_value) {
        Ok(data) => data,
        Err(err) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &err.to_string()),
    };

    let mut store = lock_store(&state);
    let existed = match store.item_exists(&collection, id) {
        Ok(value) => value,
        Err(message) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    };

    if let Err(message) = store.upsert_item(&collection, id, &data) {
        return json_error(StatusCode::INTERNAL_SERVER_ERROR, &message);
    }
    if let Err(message) = store.bump_next_id(&collection, id) {
        return json_error(StatusCode::INTERNAL_SERVER_ERROR, &message);
    }

    let status = if existed { StatusCode::OK } else { StatusCode::CREATED };
    (status, Json(response_value)).into_response()
}

#[utoipa::path(
    patch,
    path = "/{collection}/{id}",
    request_body = AnyJson,
    params(
        ("collection" = String, Path, description = "Nome da colecao"),
        ("id" = i64, Path, description = "ID do item")
    ),
    responses(
        (status = 200, body = AnyJson, description = "Item atualizado"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn patch_item(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, i64)>,
    Json(payload): Json<JsonValue>,
) -> Response {
    let patch_obj = match payload {
        JsonValue::Object(map) => map,
        _ => return json_error(StatusCode::BAD_REQUEST, "Body must be a JSON object"),
    };

    let mut store = lock_store(&state);
    let existing_data = match store.fetch_item_data(&collection, id) {
        Ok(Some(data)) => data,
        Ok(None) => return json_error(StatusCode::NOT_FOUND, "Item not found"),
        Err(message) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    };

    let mut existing_json: JsonValue = match serde_json::from_str(&existing_data) {
        Ok(value) => value,
        Err(err) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &err.to_string()),
    };

    let existing_map = match existing_json.as_object_mut() {
        Some(map) => map,
        None => return json_error(StatusCode::INTERNAL_SERVER_ERROR, "Stored value is invalid"),
    };

    for (key, value) in patch_obj {
        if key == "id" {
            continue;
        }
        existing_map.insert(key, value);
    }

    existing_map.insert("id".to_string(), JsonValue::String(id.to_string()));

    let data = match serde_json::to_string(&existing_json) {
        Ok(data) => data,
        Err(err) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &err.to_string()),
    };

    if let Err(message) = store.update_item(&collection, id, &data) {
        return json_error(StatusCode::INTERNAL_SERVER_ERROR, &message);
    }

    (StatusCode::OK, Json(existing_json)).into_response()
}

#[utoipa::path(
    delete,
    path = "/{collection}/{id}",
    params(
        ("collection" = String, Path, description = "Nome da colecao"),
        ("id" = i64, Path, description = "ID do item")
    ),
    responses(
        (status = 204, description = "Removido"),
        (status = 404, description = "Nao encontrado")
    )
)]
pub(crate) async fn delete_item(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, i64)>,
) -> Response {
    let mut store = lock_store(&state);
    let deleted = match store.delete_item(&collection, id) {
        Ok(value) => value,
        Err(message) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    };

    if !deleted {
        return json_error(StatusCode::NOT_FOUND, "Item not found");
    }

    StatusCode::NO_CONTENT.into_response()
}

fn list_items(state: AppState, collection: &str, query: ListQuery) -> Response {
    let mut store = lock_store(&state);
    let rows = match store.list_collection(collection) {
        Ok(value) => value,
        Err(message) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &message),
    };
    drop(store);

    let mut items = Vec::new();
    for data in rows {
        let json_value = match serde_json::from_str::<JsonValue>(&data) {
            Ok(value) => value,
            Err(err) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &err.to_string()),
        };
        items.push(json_value);
    }

    let mut filtered = if let Some(term) = query.term.as_ref() {
        let term_lower = term.to_lowercase();
        let fields = if query.filter.is_empty() {
            DEFAULT_FIELDS
                .iter()
                .map(|field| field.to_string())
                .collect::<Vec<_>>()
        } else {
            query.filter.clone()
        };
        items
            .into_iter()
            .filter(|item| matches_term(item, &term_lower, &fields))
            .collect::<Vec<_>>()
    } else {
        items
    };

    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(filtered.len());
    if offset < filtered.len() {
        filtered = filtered.into_iter().skip(offset).take(limit).collect();
    } else {
        filtered.clear();
    }

    (StatusCode::OK, Json(JsonValue::Array(filtered))).into_response()
}

fn matches_term(item: &JsonValue, term_lower: &str, fields: &[String]) -> bool {
    let obj = match item.as_object() {
        Some(map) => map,
        None => return false,
    };

    for field in fields {
        if let Some(value) = obj.get(field) {
            if let Some(text) = value_as_text(value) {
                if text.to_lowercase().contains(term_lower) {
                    return true;
                }
            }
        }
    }
    false
}

fn value_as_text(value: &JsonValue) -> Option<String> {
    match value {
        JsonValue::String(text) => Some(text.clone()),
        JsonValue::Number(num) => Some(num.to_string()),
        JsonValue::Bool(flag) => Some(flag.to_string()),
        JsonValue::Null => Some("null".to_string()),
        JsonValue::Array(_) | JsonValue::Object(_) => None,
    }
}

fn parse_query(raw: Option<String>) -> Result<ListQuery, Response> {
    let mut query = ListQuery::default();
    let raw = match raw {
        Some(value) if !value.trim().is_empty() => value,
        _ => return Ok(query),
    };

    let pairs: Vec<(String, String)> = serde_urlencoded::from_str(&raw)
        .map_err(|_| json_error(StatusCode::BAD_REQUEST, "Invalid query string"))?;
    for (key, value) in pairs {
        match key.as_str() {
            "term" => query.term = Some(value),
            "filter" => query.filter.push(value),
            "limit" => {
                let limit = value.parse::<usize>().map_err(|_| {
                    json_error(StatusCode::BAD_REQUEST, "Invalid limit value")
                })?;
                query.limit = Some(limit);
            }
            "offset" => {
                let offset = value.parse::<usize>().map_err(|_| {
                    json_error(StatusCode::BAD_REQUEST, "Invalid offset value")
                })?;
                query.offset = Some(offset);
            }
            _ => {}
        }
    }

    Ok(query)
}

fn json_error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}
