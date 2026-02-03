use axum::body::{to_bytes, Body};
use axum::http::{HeaderValue, Request, StatusCode};
use axum::Router;
use serde_json::Value as JsonValue;
use tower::ServiceExt;
use uuid::Uuid;

use qrud::routes::router;
use qrud::services::{AppState, Store};

async fn build_app() -> Router {
    let store = Store::open_sqlite(":memory:")
        .await
        .expect("failed to open db");
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

async fn create_workspace(app: &Router) -> String {
    create_workspace_named(app, "Main").await
}

async fn create_workspace_named(app: &Router, name: &str) -> String {
    let request = Request::builder()
        .method("POST")
        .uri("/workspaces")
        .header("content-type", "application/json")
        .body(Body::from(format!(r#"{{"name":"{}"}}"#, name)))
        .unwrap();
    let (status, json) = request_json(app, request).await;
    assert_eq!(status, StatusCode::CREATED);
    json.get("id")
        .and_then(|value| value.as_str())
        .expect("workspace id")
        .to_string()
}

#[tokio::test]
async fn workspace_crud_flow() {
    let app = build_app().await;

    let created_id = create_workspace_named(&app, "Alpha").await;
    assert!(Uuid::parse_str(&created_id).is_ok());

    let list = Request::builder()
        .method("GET")
        .uri("/workspaces")
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, list).await;
    assert_eq!(status, StatusCode::OK);
    let array = json.as_array().expect("workspace list");
    assert_eq!(array.len(), 1);

    let get = Request::builder()
        .method("GET")
        .uri(format!("/workspaces/{created_id}"))
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, get).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("Alpha"));

    let update = Request::builder()
        .method("PUT")
        .uri(format!("/workspaces/{created_id}"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Beta","description":"Team"}"#))
        .unwrap();
    let (status, json) = request_json(&app, update).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("Beta"));
    assert_eq!(
        json.get("description").and_then(|v| v.as_str()),
        Some("Team")
    );

    let patch = Request::builder()
        .method("PATCH")
        .uri(format!("/workspaces/{created_id}"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"description":"Ops"}"#))
        .unwrap();
    let (status, json) = request_json(&app, patch).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("Beta"));
    assert_eq!(
        json.get("description").and_then(|v| v.as_str()),
        Some("Ops")
    );

    let delete = Request::builder()
        .method("DELETE")
        .uri(format!("/workspaces/{created_id}"))
        .body(Body::empty())
        .unwrap();
    let status = request_status(&app, delete).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let get = Request::builder()
        .method("GET")
        .uri(format!("/workspaces/{created_id}"))
        .body(Body::empty())
        .unwrap();
    let status = request_status(&app, get).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn workspace_validation_errors() {
    let app = build_app().await;

    let create = Request::builder()
        .method("POST")
        .uri("/workspaces")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"   "}"#))
        .unwrap();
    let status = request_status(&app, create).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let workspace_id = create_workspace(&app).await;
    let patch = Request::builder()
        .method("PATCH")
        .uri(format!("/workspaces/{workspace_id}"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{}"#))
        .unwrap();
    let status = request_status(&app, patch).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn post_document_ignores_payload_id() {
    let app = build_app().await;
    let workspace_id = create_workspace(&app).await;

    let request = Request::builder()
        .method("POST")
        .uri(format!("/workspaces/{workspace_id}/documents/users"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"should-ignore","name":"Ana"}"#))
        .unwrap();

    let (status, json) = request_json(&app, request).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = json.get("id").and_then(|v| v.as_str()).expect("id");
    assert_ne!(id, "should-ignore");
    assert!(Uuid::parse_str(id).is_ok());
    assert!(json
        .get("data")
        .and_then(|v| v.get("id"))
        .is_none());
}

#[tokio::test]
async fn put_document_creates_and_updates() {
    let app = build_app().await;
    let workspace_id = create_workspace(&app).await;

    let request = Request::builder()
        .method("PUT")
        .uri(format!("/workspaces/{workspace_id}/documents/users"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Bea"}"#))
        .unwrap();

    let (status, json) = request_json(&app, request).await;
    assert_eq!(status, StatusCode::CREATED);
    let first_id = json.get("id").and_then(|v| v.as_str()).unwrap();

    let update = Request::builder()
        .method("PUT")
        .uri(format!("/workspaces/{workspace_id}/documents/users"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Carlos"}"#))
        .unwrap();

    let (status, json) = request_json(&app, update).await;
    assert_eq!(status, StatusCode::OK);
    let updated_id = json.get("id").and_then(|v| v.as_str()).unwrap();
    assert_eq!(first_id, updated_id);
    assert_eq!(
        json.get("data").and_then(|v| v.get("name")).and_then(|v| v.as_str()),
        Some("Carlos")
    );
}

#[tokio::test]
async fn patch_document_merges_fields() {
    let app = build_app().await;
    let workspace_id = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri(format!("/workspaces/{workspace_id}/documents/products"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Car","description":"Base"}"#))
        .unwrap();
    let (_, created) = request_json(&app, create).await;
    let id = created.get("id").and_then(|v| v.as_str()).unwrap().to_string();

    let patch = Request::builder()
        .method("PATCH")
        .uri(format!("/workspaces/{workspace_id}/documents/products"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"description":"Updated","id":"999"}"#))
        .unwrap();
    let (status, json) = request_json(&app, patch).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("id").and_then(|v| v.as_str()), Some(id.as_str()));
    assert_eq!(
        json.get("data")
            .and_then(|v| v.get("description"))
            .and_then(|v| v.as_str()),
        Some("Updated")
    );
    assert_eq!(
        json.get("data")
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str()),
        Some("Car")
    );
}

#[tokio::test]
async fn delete_document_then_get_returns_404() {
    let app = build_app().await;
    let workspace_id = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri(format!("/workspaces/{workspace_id}/documents/sessions"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"One"}"#))
        .unwrap();
    request_json(&app, create).await;

    let delete = Request::builder()
        .method("DELETE")
        .uri(format!("/workspaces/{workspace_id}/documents/sessions"))
        .body(Body::empty())
        .unwrap();
    let status = request_status(&app, delete).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let get = Request::builder()
        .method("GET")
        .uri(format!("/workspaces/{workspace_id}/documents/sessions"))
        .body(Body::empty())
        .unwrap();
    let status = request_status(&app, get).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn header_workspace_routes_work() {
    let app = build_app().await;
    let workspace_id = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri("/documents/users")
        .header("x-workspace-id", &workspace_id)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let (status, _) = request_json(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);

    let get = Request::builder()
        .method("GET")
        .uri("/documents/users")
        .header("x-workspace-id", &workspace_id)
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, get).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json.get("data").and_then(|v| v.get("name")).and_then(|v| v.as_str()),
        Some("Ana")
    );
}

#[tokio::test]
async fn header_workspace_missing_returns_400() {
    let app = build_app().await;

    let create = Request::builder()
        .method("POST")
        .uri("/documents/users")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let status = request_status(&app, create).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn document_requires_existing_workspace() {
    let app = build_app().await;
    let fake_id = Uuid::now_v7().to_string();

    let create = Request::builder()
        .method("POST")
        .uri(format!("/workspaces/{fake_id}/documents/users"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let status = request_status(&app, create).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn document_pk_is_normalized() {
    let app = build_app().await;
    let workspace_id = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri(format!("/workspaces/{workspace_id}/documents/users"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let (status, json) = request_json(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json.get("pk").and_then(|v| v.as_str()), Some("/users"));
}

#[tokio::test]
async fn document_id_is_uuid_v7() {
    let app = build_app().await;
    let workspace_id = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri(format!("/workspaces/{workspace_id}/documents/users"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let (status, json) = request_json(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = json.get("id").and_then(|v| v.as_str()).expect("id");
    let uuid = Uuid::parse_str(id).expect("uuid");
    assert_eq!(uuid.get_version(), Some(uuid::Version::SortRand));
}

#[tokio::test]
async fn get_document_success() {
    let app = build_app().await;
    let workspace_id = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri(format!("/workspaces/{workspace_id}/documents/users"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    request_json(&app, create).await;

    let get = Request::builder()
        .method("GET")
        .uri(format!("/workspaces/{workspace_id}/documents/users"))
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, get).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json.get("data").and_then(|v| v.get("name")).and_then(|v| v.as_str()),
        Some("Ana")
    );
}

#[tokio::test]
async fn document_post_conflict_returns_409() {
    let app = build_app().await;
    let workspace_id = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri(format!("/workspaces/{workspace_id}/documents/users"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let status = request_status(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);

    let duplicate = Request::builder()
        .method("POST")
        .uri(format!("/workspaces/{workspace_id}/documents/users"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let status = request_status(&app, duplicate).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn header_document_put_patch_delete() {
    let app = build_app().await;
    let workspace_id = create_workspace(&app).await;

    let put = Request::builder()
        .method("PUT")
        .uri("/documents/users")
        .header("x-workspace-id", &workspace_id)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let (status, json) = request_json(&app, put).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = json.get("id").and_then(|v| v.as_str()).unwrap().to_string();

    let patch = Request::builder()
        .method("PATCH")
        .uri("/documents/users")
        .header("x-workspace-id", &workspace_id)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"role":"admin"}"#))
        .unwrap();
    let (status, json) = request_json(&app, patch).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("id").and_then(|v| v.as_str()), Some(id.as_str()));

    let delete = Request::builder()
        .method("DELETE")
        .uri("/documents/users")
        .header("x-workspace-id", &workspace_id)
        .body(Body::empty())
        .unwrap();
    let status = request_status(&app, delete).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn header_workspace_invalid_value_returns_400() {
    let app = build_app().await;

    let mut builder = Request::builder();
    builder = builder.method("GET").uri("/documents/users");
    let request = builder
        .header(
            "x-workspace-id",
            HeaderValue::from_bytes(b"\xFF").unwrap(),
        )
        .body(Body::empty())
        .unwrap();
    let status = request_status(&app, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn openapi_route_available() {
    let app = build_app().await;
    let request = Request::builder()
        .method("GET")
        .uri("/openapi.json")
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, request).await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("openapi").is_some());
}
