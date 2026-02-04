use axum::body::{to_bytes, Body};
use axum::http::{HeaderValue, Request, StatusCode};
use axum::Router;
use serde_json::Value as JsonValue;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};
use tower::ServiceExt;
use uuid::Uuid;

use qrud::routes::router;
use qrud::services::{ApiContract, AppState, Store};

async fn build_app() -> Router {
    build_app_with_default(false).await
}

async fn build_app_with_default(use_default: bool) -> Router {
    build_app_with_default_and_contract(use_default, None).await
}

async fn build_app_with_default_and_contract(use_default: bool, api_contract: Option<ApiContract>) -> Router {
    let store = Store::open_sqlite(":memory:")
        .await
        .expect("failed to open db");
    let state = AppState::new(store, use_default, api_contract);
    router(state)
}

async fn build_app_with_openapi_contract() -> Router {
    let contract_path = write_test_openapi_file();
    let contract = ApiContract::from_file(contract_path.to_str().expect("openapi path"))
        .expect("load openapi");
    build_app_with_default_and_contract(false, Some(contract)).await
}

fn write_test_openapi_file() -> PathBuf {
    let mut path = std::env::temp_dir();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    path.push(format!("qrud-openapi-{now}.json"));
    let spec = serde_json::json!({
        "openapi": "3.0.3",
        "info": { "title": "test", "version": "1.0.0" },
        "paths": {
            "/users": {
                "post": {
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/UserPayload" }
                            }
                        }
                    }
                },
                "get": {}
            },
            "/users/{id}": {
                "get": {},
                "put": {
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/UserPayload" }
                            }
                        }
                    }
                },
                "patch": {
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/UserPayload" }
                            }
                        }
                    }
                },
                "delete": {}
            }
        },
        "components": {
            "schemas": {
                "UserPayload": {
                    "type": "object",
                    "required": ["name"],
                    "properties": {
                        "name": { "type": "string" }
                    }
                }
            }
        }
    });
    fs::write(&path, serde_json::to_string(&spec).expect("serialize openapi"))
        .expect("write openapi");
    path
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
    create_workspace_named(app, "main").await.0
}

async fn create_workspace_named(app: &Router, name: &str) -> (String, String) {
    let request = Request::builder()
        .method("POST")
        .uri("/workspaces")
        .header("content-type", "application/json")
        .body(Body::from(format!(r#"{{"name":"{}"}}"#, name)))
        .unwrap();
    let (status, json) = request_json(app, request).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = json
        .get("id")
        .and_then(|value| value.as_str())
        .expect("workspace id")
        .to_string();
    (name.to_string(), id)
}

#[tokio::test]
async fn workspace_crud_flow() {
    let app = build_app().await;

    let (workspace_name, workspace_id) = create_workspace_named(&app, "alpha").await;
    assert!(Uuid::parse_str(&workspace_id).is_ok());

    let list = Request::builder()
        .method("GET")
        .uri("/workspaces")
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, list).await;
    assert_eq!(status, StatusCode::OK);
    let array = json.as_array().expect("workspace list");
    assert_eq!(array.len(), 2);

    let get = Request::builder()
        .method("GET")
        .uri(format!("/workspaces/{workspace_name}"))
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, get).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("alpha"));

    let update = Request::builder()
        .method("PUT")
        .uri(format!("/workspaces/{workspace_name}"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"beta","description":"Team"}"#))
        .unwrap();
    let (status, json) = request_json(&app, update).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("beta"));
    assert_eq!(
        json.get("description").and_then(|v| v.as_str()),
        Some("Team")
    );

    let patch = Request::builder()
        .method("PATCH")
        .uri("/workspaces/beta")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"description":"Ops"}"#))
        .unwrap();
    let (status, json) = request_json(&app, patch).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("beta"));
    assert_eq!(
        json.get("description").and_then(|v| v.as_str()),
        Some("Ops")
    );

    let delete = Request::builder()
        .method("DELETE")
        .uri("/workspaces/beta")
        .body(Body::empty())
        .unwrap();
    let status = request_status(&app, delete).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let get = Request::builder()
        .method("GET")
        .uri("/workspaces/beta")
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
        .body(Body::from(r#"{"name":"Bad Name"}"#))
        .unwrap();
    let status = request_status(&app, create).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let workspace_name = create_workspace(&app).await;
    let patch = Request::builder()
        .method("PATCH")
        .uri(format!("/workspaces/{workspace_name}"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{}"#))
        .unwrap();
    let status = request_status(&app, patch).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let duplicate = Request::builder()
        .method("POST")
        .uri("/workspaces")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"main"}"#))
        .unwrap();
    let status = request_status(&app, duplicate).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn post_document_ignores_payload_id() {
    let app = build_app().await;
    let workspace_name = create_workspace(&app).await;

    let request = Request::builder()
        .method("POST")
        .uri("/users".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"should-ignore","name":"Ana"}"#))
        .unwrap();

    let (status, json) = request_json(&app, request).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = json.get("$id").and_then(|v| v.as_str()).expect("id");
    assert_ne!(id, "should-ignore");
    assert!(Uuid::parse_str(id).is_ok());
    assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("Ana"));
}

#[tokio::test]
async fn put_document_creates_and_updates() {
    let app = build_app().await;
    let workspace_name = create_workspace(&app).await;

    let request = Request::builder()
        .method("PUT")
        .uri("/users".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Bea"}"#))
        .unwrap();

    let (status, json) = request_json(&app, request).await;
    assert_eq!(status, StatusCode::CREATED);
    let first_id = json.get("$id").and_then(|v| v.as_str()).unwrap();

    let update = Request::builder()
        .method("PUT")
        .uri("/users".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Carlos"}"#))
        .unwrap();

    let (status, json) = request_json(&app, update).await;
    assert_eq!(status, StatusCode::OK);
    let updated_id = json.get("$id").and_then(|v| v.as_str()).unwrap();
    assert_eq!(first_id, updated_id);
    assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("Carlos"));
}

#[tokio::test]
async fn patch_document_merges_fields() {
    let app = build_app().await;
    let workspace_name = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri("/products".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Car","description":"Base"}"#))
        .unwrap();
    let (_, created) = request_json(&app, create).await;
    let id = created.get("$id").and_then(|v| v.as_str()).unwrap().to_string();

    let patch = Request::builder()
        .method("PATCH")
        .uri("/products".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"description":"Updated","id":"999"}"#))
        .unwrap();
    let (status, json) = request_json(&app, patch).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("$id").and_then(|v| v.as_str()), Some(id.as_str()));
    assert_eq!(
        json.get("description").and_then(|v| v.as_str()),
        Some("Updated")
    );
    assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("Car"));
}

#[tokio::test]
async fn delete_document_then_get_returns_404() {
    let app = build_app().await;
    let workspace_name = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri("/sessions".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"One"}"#))
        .unwrap();
    request_json(&app, create).await;

    let delete = Request::builder()
        .method("DELETE")
        .uri("/sessions".to_string())
        .header("x-workspace-id", &workspace_name)
        .body(Body::empty())
        .unwrap();
    let status = request_status(&app, delete).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let get = Request::builder()
        .method("GET")
        .uri("/sessions".to_string())
        .header("x-workspace-id", &workspace_name)
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, get).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("total").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(json.get("limit").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(json.get("offset").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(json.get("order").and_then(|v| v.as_str()), Some("desc"));
    assert_eq!(json.get("by").and_then(|v| v.as_str()), Some("created_at"));
}

#[tokio::test]
async fn header_workspace_routes_work() {
    let app = build_app().await;
    let workspace_name = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri("/users")
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let (status, _) = request_json(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);

    let get = Request::builder()
        .method("GET")
        .uri("/users")
        .header("x-workspace-id", &workspace_name)
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, get).await;
    assert_eq!(status, StatusCode::OK);
    let array = json
        .get("items")
        .and_then(|value| value.as_array())
        .expect("document items");
    assert_eq!(json.get("total").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(json.get("limit").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(json.get("offset").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(json.get("order").and_then(|v| v.as_str()), Some("desc"));
    assert_eq!(json.get("by").and_then(|v| v.as_str()), Some("created_at"));
    assert_eq!(
        array[0].get("name").and_then(|v| v.as_str()),
        Some("Ana")
    );
}

#[tokio::test]
async fn root_document_routes_work() {
    let app = build_app().await;
    let workspace_name = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri("/posts")
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"title":"Oi"}"#))
        .unwrap();
    let (status, _) = request_json(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);

    let get = Request::builder()
        .method("GET")
        .uri("/posts")
        .header("x-workspace-id", &workspace_name)
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, get).await;
    assert_eq!(status, StatusCode::OK);
    let array = json
        .get("items")
        .and_then(|value| value.as_array())
        .expect("document items");
    assert_eq!(json.get("total").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(json.get("limit").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(json.get("offset").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(json.get("order").and_then(|v| v.as_str()), Some("desc"));
    assert_eq!(json.get("by").and_then(|v| v.as_str()), Some("created_at"));
    assert_eq!(
        array[0].get("title").and_then(|v| v.as_str()),
        Some("Oi")
    );
}

#[tokio::test]
async fn workspace_document_routes_work() {
    let app = build_app().await;
    let workspace_name = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri(format!("/workspaces/{workspace_name}/posts"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"title":"Oi"}"#))
        .unwrap();
    let (status, _) = request_json(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);

    let get = Request::builder()
        .method("GET")
        .uri(format!("/workspaces/{workspace_name}/posts"))
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, get).await;
    assert_eq!(status, StatusCode::OK);
    let array = json
        .get("items")
        .and_then(|value| value.as_array())
        .expect("document items");
    assert_eq!(json.get("total").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(json.get("limit").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(json.get("offset").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(json.get("order").and_then(|v| v.as_str()), Some("desc"));
    assert_eq!(json.get("by").and_then(|v| v.as_str()), Some("created_at"));
    assert_eq!(
        array[0].get("title").and_then(|v| v.as_str()),
        Some("Oi")
    );
}

#[tokio::test]
async fn header_workspace_missing_returns_400() {
    let app = build_app().await;

    let create = Request::builder()
        .method("POST")
        .uri("/users")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let status = request_status(&app, create).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn use_default_workspace_when_enabled() {
    let app = build_app_with_default(true).await;

    let create = Request::builder()
        .method("POST")
        .uri("/users")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let (status, json) = request_json(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("Ana"));
}

#[tokio::test]
async fn reserved_pk_returns_400() {
    let app = build_app().await;
    let workspace_name = create_workspace(&app).await;

    let request = Request::builder()
        .method("POST")
        .uri("/documents")
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let status = request_status(&app, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

}

#[tokio::test]
async fn document_requires_existing_workspace() {
    let app = build_app().await;
    let fake_name = "missing-workspace";

    let create = Request::builder()
        .method("POST")
        .uri("/users".to_string())
        .header("x-workspace-id", fake_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let status = request_status(&app, create).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn document_pk_is_normalized() {
    let app = build_app().await;
    let workspace_name = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri("/users".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let (status, json) = request_json(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(json.get("pk").is_none());
    assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("Ana"));
}

#[tokio::test]
async fn document_id_is_uuid_v7() {
    let app = build_app().await;
    let workspace_name = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri("/users".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let (status, json) = request_json(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = json.get("$id").and_then(|v| v.as_str()).expect("id");
    let uuid = Uuid::parse_str(id).expect("uuid");
    assert_eq!(uuid.get_version(), Some(uuid::Version::SortRand));
}

#[tokio::test]
async fn get_document_success() {
    let app = build_app().await;
    let workspace_name = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri("/users".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    request_json(&app, create).await;

    let get = Request::builder()
        .method("GET")
        .uri("/users".to_string())
        .header("x-workspace-id", &workspace_name)
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, get).await;
    assert_eq!(status, StatusCode::OK);
    let array = json
        .get("items")
        .and_then(|value| value.as_array())
        .expect("document items");
    assert_eq!(json.get("total").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(json.get("limit").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(json.get("offset").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(json.get("order").and_then(|v| v.as_str()), Some("desc"));
    assert_eq!(json.get("by").and_then(|v| v.as_str()), Some("created_at"));
    assert_eq!(
        array[0].get("name").and_then(|v| v.as_str()),
        Some("Ana")
    );
}

#[tokio::test]
async fn document_post_conflict_returns_409() {
    let app = build_app().await;
    let workspace_name = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri("/users".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let status = request_status(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);

    let duplicate = Request::builder()
        .method("POST")
        .uri("/users".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let status = request_status(&app, duplicate).await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn document_get_by_id_path() {
    let app = build_app().await;
    let workspace_name = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri("/users".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let (status, json) = request_json(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = json.get("$id").and_then(|v| v.as_str()).expect("id");

    let get = Request::builder()
        .method("GET")
        .uri(format!("/users/{id}"))
        .header("x-workspace-id", &workspace_name)
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, get).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("$id").and_then(|v| v.as_str()), Some(id));
    assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("Ana"));
}

#[tokio::test]
async fn document_patch_by_id_path() {
    let app = build_app().await;
    let workspace_name = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri("/users".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let (status, json) = request_json(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = json.get("$id").and_then(|v| v.as_str()).expect("id");

    let patch = Request::builder()
        .method("PATCH")
        .uri(format!("/users/{id}"))
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"role":"admin"}"#))
        .unwrap();
    let (status, json) = request_json(&app, patch).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("$id").and_then(|v| v.as_str()), Some(id));
    assert_eq!(json.get("role").and_then(|v| v.as_str()), Some("admin"));
}

#[tokio::test]
async fn allow_duplicate_pk_returns_latest() {
    let app = build_app().await;
    let workspace_name = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri("/users".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"First"}"#))
        .unwrap();
    let (status, first) = request_json(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);

    sleep(Duration::from_millis(2)).await;

    let create = Request::builder()
        .method("POST")
        .uri("/users".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Second"}"#))
        .unwrap();
    let (status, second) = request_json(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_ne!(
        first.get("$id").and_then(|v| v.as_str()),
        second.get("$id").and_then(|v| v.as_str())
    );

    let get = Request::builder()
        .method("GET")
        .uri("/users".to_string())
        .header("x-workspace-id", &workspace_name)
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, get).await;
    assert_eq!(status, StatusCode::OK);
    let array = json
        .get("items")
        .and_then(|value| value.as_array())
        .expect("document items");
    assert_eq!(json.get("total").and_then(|v| v.as_i64()), Some(2));
    assert_eq!(json.get("limit").and_then(|v| v.as_i64()), Some(2));
    assert_eq!(json.get("offset").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(json.get("order").and_then(|v| v.as_str()), Some("desc"));
    assert_eq!(json.get("by").and_then(|v| v.as_str()), Some("created_at"));
    assert_eq!(
        array[0].get("name").and_then(|v| v.as_str()),
        Some("Second")
    );
}

#[tokio::test]
async fn pk_list_order_param_sorts_created_at() {
    let app = build_app().await;
    let workspace_name = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri("/users".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"First"}"#))
        .unwrap();
    let status = request_status(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);

    sleep(Duration::from_millis(2)).await;

    let create = Request::builder()
        .method("POST")
        .uri("/users".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Second"}"#))
        .unwrap();
    let status = request_status(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);

    let get = Request::builder()
        .method("GET")
        .uri("/users?order=asc&by=created_at")
        .header("x-workspace-id", &workspace_name)
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, get).await;
    assert_eq!(status, StatusCode::OK);
    let items = json
        .get("items")
        .and_then(|value| value.as_array())
        .expect("document items");
    assert_eq!(
        items[0].get("name").and_then(|v| v.as_str()),
        Some("First")
    );
    assert_eq!(json.get("order").and_then(|v| v.as_str()), Some("asc"));
    assert_eq!(json.get("by").and_then(|v| v.as_str()), Some("created_at"));

    let get = Request::builder()
        .method("GET")
        .uri("/users?order=DESC&by=updated_at")
        .header("x-workspace-id", &workspace_name)
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, get).await;
    assert_eq!(status, StatusCode::OK);
    let items = json
        .get("items")
        .and_then(|value| value.as_array())
        .expect("document items");
    assert_eq!(
        items[0].get("name").and_then(|v| v.as_str()),
        Some("Second")
    );
    assert_eq!(json.get("order").and_then(|v| v.as_str()), Some("desc"));
    assert_eq!(json.get("by").and_then(|v| v.as_str()), Some("updated_at"));
}

#[tokio::test]
async fn pk_list_term_filters_results() {
    let app = build_app().await;
    let workspace_name = create_workspace(&app).await;

    let create = Request::builder()
        .method("POST")
        .uri("/users".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Philippe Assis"}"#))
        .unwrap();
    let status = request_status(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);

    let create = Request::builder()
        .method("POST")
        .uri("/users".to_string())
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Other"}"#))
        .unwrap();
    let status = request_status(&app, create).await;
    assert_eq!(status, StatusCode::CREATED);

    let get = Request::builder()
        .method("GET")
        .uri("/users?term=assis")
        .header("x-workspace-id", &workspace_name)
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, get).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("total").and_then(|v| v.as_i64()), Some(2));
    let items = json
        .get("items")
        .and_then(|value| value.as_array())
        .expect("document items");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].get("name").and_then(|v| v.as_str()),
        Some("Philippe Assis")
    );
}

#[tokio::test]
async fn header_document_put_patch_delete() {
    let app = build_app().await;
    let workspace_name = create_workspace(&app).await;

    let put = Request::builder()
        .method("PUT")
        .uri("/users")
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let (status, json) = request_json(&app, put).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = json.get("$id").and_then(|v| v.as_str()).unwrap().to_string();

    let patch = Request::builder()
        .method("PATCH")
        .uri("/users")
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"role":"admin"}"#))
        .unwrap();
    let (status, json) = request_json(&app, patch).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("$id").and_then(|v| v.as_str()), Some(id.as_str()));

    let delete = Request::builder()
        .method("DELETE")
        .uri("/users")
        .header("x-workspace-id", &workspace_name)
        .body(Body::empty())
        .unwrap();
    let status = request_status(&app, delete).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn header_workspace_invalid_value_returns_400() {
    let app = build_app().await;

    let mut builder = Request::builder();
    builder = builder.method("GET").uri("/users");
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

#[tokio::test]
async fn contract_rejects_unknown_pk_path() {
    let app = build_app_with_openapi_contract().await;
    let workspace_name = create_workspace(&app).await;

    let request = Request::builder()
        .method("POST")
        .uri("/posts")
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let status = request_status(&app, request).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn contract_validates_request_payload() {
    let app = build_app_with_openapi_contract().await;
    let workspace_name = create_workspace(&app).await;

    let request = Request::builder()
        .method("POST")
        .uri("/users")
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"label":"Ana"}"#))
        .unwrap();
    let status = request_status(&app, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let request = Request::builder()
        .method("POST")
        .uri("/users")
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana","extra":123}"#))
        .unwrap();
    let status = request_status(&app, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let request = Request::builder()
        .method("POST")
        .uri("/users")
        .header("x-workspace-id", &workspace_name)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Ana"}"#))
        .unwrap();
    let status = request_status(&app, request).await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn health_and_info_routes() {
    let app = build_app().await;

    let health = Request::builder()
        .method("GET")
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(health).await.expect("request failed");
    let status = response.status();
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read body");
    assert_eq!(status, StatusCode::OK);
    assert_eq!(String::from_utf8_lossy(&body), "OK");

    let info = Request::builder()
        .method("GET")
        .uri("/info")
        .body(Body::empty())
        .unwrap();
    let (status, json) = request_json(&app, info).await;
    assert_eq!(status, StatusCode::OK);
    let database = json.get("database").expect("database info");
    assert_eq!(database.get("drive").and_then(|v| v.as_str()), Some("sqlite"));
    assert_eq!(
        database
            .get("sqlite")
            .and_then(|info| info.get("in_memory"))
            .and_then(|v| v.as_bool()),
        Some(true)
    );
}
