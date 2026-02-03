use axum::extract::{Path, RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post, put};
use axum::{Json, Router};
use serde_json::{json, Value as JsonValue};

use crate::models::{ListQuery, DEFAULT_FIELDS};
use crate::services::{lock_store, AppState};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/{*path}", get(handle_get))
        .route("/{*path}", post(handle_post))
        .route("/{*path}", put(handle_put))
        .route("/{*path}", patch(handle_patch))
        .route("/{*path}", delete(handle_delete))
        .with_state(state)
}

async fn handle_get(
    State(state): State<AppState>,
    Path(path): Path<String>,
    RawQuery(raw): RawQuery,
) -> Response {
    let segments = split_path(&path);
    if segments.is_empty() {
        return json_error(StatusCode::NOT_FOUND, "Not found");
    }

    let query = match parse_query(raw) {
        Ok(query) => query,
        Err(resp) => return resp,
    };

    if let Some((collection, id)) = parse_item_target(&segments) {
        return get_item(state, &collection, id);
    }

    let collection = segments.join("/");
    list_items(state, &collection, query)
}

async fn handle_post(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Json(payload): Json<JsonValue>,
) -> Response {
    let segments = split_path(&path);
    if segments.is_empty() {
        return json_error(StatusCode::NOT_FOUND, "Not found");
    }
    let collection = segments.join("/");

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

async fn handle_put(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Json(payload): Json<JsonValue>,
) -> Response {
    let (collection, id) = match parse_item_target_for_write(&path) {
        Ok(target) => target,
        Err(resp) => return resp,
    };

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

async fn handle_patch(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Json(payload): Json<JsonValue>,
) -> Response {
    let (collection, id) = match parse_item_target_for_write(&path) {
        Ok(target) => target,
        Err(resp) => return resp,
    };

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

async fn handle_delete(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Response {
    let (collection, id) = match parse_item_target_for_write(&path) {
        Ok(target) => target,
        Err(resp) => return resp,
    };

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

fn get_item(state: AppState, collection: &str, id: i64) -> Response {
    let mut store = lock_store(&state);
    let data = match store.fetch_item_data(collection, id) {
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

fn split_path(path: &str) -> Vec<&str> {
    path.split('/').filter(|segment| !segment.is_empty()).collect()
}

fn parse_item_target(segments: &[&str]) -> Option<(String, i64)> {
    if segments.len() < 2 {
        return None;
    }
    let id = segments[segments.len() - 1];
    if !is_numeric_id(id) {
        return None;
    }
    let id_num = id.parse::<i64>().ok()?;
    let collection = segments[..segments.len() - 1].join("/");
    Some((collection, id_num))
}

fn parse_item_target_for_write(path: &str) -> Result<(String, i64), Response> {
    let segments = split_path(path);
    if segments.len() < 2 {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "Item path must include an id",
        ));
    }
    let id = segments[segments.len() - 1];
    if !is_numeric_id(id) {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "Id must be numeric",
        ));
    }
    let id_num = id
        .parse::<i64>()
        .map_err(|_| json_error(StatusCode::BAD_REQUEST, "Id must be numeric"))?;
    let collection = segments[..segments.len() - 1].join("/");
    Ok((collection, id_num))
}

fn is_numeric_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|ch| ch.is_ascii_digit())
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

fn json_error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
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
