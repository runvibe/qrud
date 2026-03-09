mod common;
mod documents;
mod logging;
mod openapi;
mod workspaces;

use std::sync::Arc;

use axum::routing::get;
use axum::{Extension, Json, Router, middleware};
use utoipa::openapi::{ContactBuilder, InfoBuilder, LicenseBuilder, OpenApiBuilder, Paths};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::services::AppState;

pub fn router(state: AppState) -> Router {
    let (router, api) = build_openapi_router().split_for_parts();
    let api_json = Arc::new(serde_json::to_value(&api).expect("failed to serialize openapi"));
    let openapi_state = state.clone();

    Router::new()
        .merge(router)
        .route(
            "/openapi.json",
            get({
                let api_json = api_json.clone();
                move || {
                    let api_json = api_json.clone();
                    let state = openapi_state.clone();
                    async move {
                        let body = state
                            .api_contract()
                            .map(|contract| contract.openapi_spec().clone())
                            .unwrap_or_else(|| api_json.as_ref().clone());
                        Json(body)
                    }
                }
            }),
        )
        .layer(Extension(state))
        .layer(middleware::from_fn(logging::log_request_response))
}

fn build_openapi_router() -> OpenApiRouter {
    OpenApiRouter::with_openapi(build_openapi())
        .routes(routes!(
            workspaces::list_workspaces,
            workspaces::create_workspace
        ))
        .routes(routes!(
            workspaces::get_workspace,
            workspaces::put_workspace,
            workspaces::patch_workspace,
            workspaces::delete_workspace
        ))
        .routes(routes!(
            documents::get_document_root,
            documents::create_document_root,
            documents::put_document_root,
            documents::patch_document_root,
            documents::delete_document_root
        ))
        .routes(routes!(
            documents::get_document_workspace,
            documents::create_document_workspace,
            documents::put_document_workspace,
            documents::patch_document_workspace,
            documents::delete_document_workspace
        ))
        .routes(routes!(openapi::put_contract, openapi::delete_contract))
        .routes(routes!(documents::health))
        .routes(routes!(documents::info))
}

fn build_openapi() -> utoipa::openapi::OpenApi {
    let description = env!("CARGO_PKG_DESCRIPTION").trim();
    let description = if description.is_empty() {
        None
    } else {
        Some(description.to_string())
    };

    let info = InfoBuilder::new()
        .title(format!("{} API", env!("CARGO_PKG_NAME")))
        .version(env!("CARGO_PKG_VERSION"))
        .description(description)
        .contact(cargo_contact())
        .license(Some(
            LicenseBuilder::new()
                .name("Apache-2.0")
                .url(Some("https://www.apache.org/licenses/LICENSE-2.0"))
                .build(),
        ))
        .build();

    OpenApiBuilder::new().info(info).paths(Paths::new()).build()
}

fn cargo_contact() -> Option<utoipa::openapi::Contact> {
    let (name, email) = parse_author(env!("CARGO_PKG_AUTHORS"));
    if name.is_none() && email.is_none() {
        return None;
    }

    Some(ContactBuilder::new().name(name).email(email).build())
}

fn parse_author(authors: &str) -> (Option<String>, Option<String>) {
    let first = authors.split(':').next().unwrap_or_default().trim();
    if first.is_empty() {
        return (None, None);
    }

    if let Some((name, rest)) = first.split_once('<') {
        let name = name.trim();
        let email = rest.trim_end_matches('>').trim();
        return (to_option(name), to_option(email));
    }

    (to_option(first), None)
}

fn to_option(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}
