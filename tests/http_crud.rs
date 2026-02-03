use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::Value as JsonValue;
use tower::ServiceExt;

use qrud::routes::router;
use qrud::services::{AppState, Store};

fn build_app() -> Router {
    let store = Store::open(":memory:").expect("failed to open db");
    let state = AppState::new(store);
    router(state)
}

async fn request_json(app: &Router, request: Request<Body>) -> (StatusCode, JsonValue) {
    let response = app.clone().oneshot(request).await.expect("request failed");
    let status = response.status();
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read body");
    let json = serde_json::from_slice(&body).unwrap_or_else(|err| {
        let body_str = String::from_utf8_lossy(&body);
        panic!("json body (status {status}): {err}; body={body_str}");
    });
    (status, json)
}

async fn request_status(app: &Router, request: Request<Body>) -> StatusCode {
    let response = app.clone().oneshot(request).await.expect("request failed");
    response.status()
}

#[tokio::test]
async fn post_ignores_payload_id_and_autoincrements() {
    let app = build_app();
    let request = Request::builder()
        .method("POST")
        .uri("/users")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":999,"name":"Ana"}"#))
        .unwrap();

    let (status, json) = request_json(&app, request).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json.get("id").and_then(|v| v.as_str()), Some("1"));
    assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("Ana"));
}

#[tokio::test]
async fn put_creates_and_bumps_counter_for_next_post() {
    let app = build_app();
    let request = Request::builder()
        .method("PUT")
        .uri("/users/5")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Bea"}"#))
        .unwrap();

    let (status, json) = request_json(&app, request).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json.get("id").and_then(|v| v.as_str()), Some("5"));

    let post_request = Request::builder()
        .method("POST")
        .uri("/users")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Carlos"}"#))
        .unwrap();

    let (status, json) = request_json(&app, post_request).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json.get("id").and_then(|v| v.as_str()), Some("6"));
}

#[tokio::test]
async fn patch_merges_fields_and_keeps_id() {
    let app = build_app();
    let create = Request::builder()
        .method("POST")
        .uri("/products")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Car","description":"Base"}"#))
        .unwrap();
    let (_, created) = request_json(&app, create).await;
    let id = created
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();

    let patch = Request::builder()
        .method("PATCH")
        .uri(format!("/products/{id}"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"description":"Updated","id":"999"}"#))
        .unwrap();
    let (status, json) = request_json(&app, patch).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("id").and_then(|v| v.as_str()), Some(id.as_str()));
    assert_eq!(
        json.get("description").and_then(|v| v.as_str()),
        Some("Updated")
    );
}

#[tokio::test]
async fn list_filters_and_paginates() {
    let app = build_app();
    let create1 = Request::builder()
        .method("POST")
        .uri("/items")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Car","description":"fast car"}"#))
        .unwrap();
    let create2 = Request::builder()
        .method("POST")
        .uri("/items")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Bike","description":"fast bike"}"#))
        .unwrap();
    let create3 = Request::builder()
        .method("POST")
        .uri("/items")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Cart","description":"slow cart"}"#))
        .unwrap();

    request_json(&app, create1).await;
    request_json(&app, create2).await;
    request_json(&app, create3).await;

    let list = Request::builder()
        .method("GET")
        .uri("/items?term=car&filter=name&limit=10&offset=0")
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, list).await;
    assert_eq!(status, StatusCode::OK);
    let array = json.as_array().expect("array result");
    assert_eq!(array.len(), 2);

    let list_page = Request::builder()
        .method("GET")
        .uri("/items?term=car&filter=name&limit=1&offset=1")
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, list_page).await;
    assert_eq!(status, StatusCode::OK);
    let array = json.as_array().expect("array result");
    assert_eq!(array.len(), 1);
}

#[tokio::test]
async fn delete_then_get_returns_404() {
    let app = build_app();
    let create = Request::builder()
        .method("POST")
        .uri("/sessions")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"One"}"#))
        .unwrap();
    let (_, created) = request_json(&app, create).await;
    let id = created
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();

    let delete = Request::builder()
        .method("DELETE")
        .uri(format!("/sessions/{id}"))
        .body(Body::empty())
        .unwrap();
    let status = request_status(&app, delete).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let get = Request::builder()
        .method("GET")
        .uri(format!("/sessions/{id}"))
        .body(Body::empty())
        .unwrap();
    let status = request_status(&app, get).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
