mod items;
mod logging;

use std::sync::Arc;

use axum::routing::get;
use axum::{middleware, Extension, Json, Router};
use utoipa::openapi::{ContactBuilder, InfoBuilder, LicenseBuilder, OpenApiBuilder, Paths};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::services::AppState;

pub fn router(state: AppState) -> Router {
    let (router, api) = OpenApiRouter::with_openapi(build_openapi())
        .routes(routes!(items::list_workspaces, items::create_workspace))
        .routes(routes!(
            items::get_workspace,
            items::put_workspace,
            items::patch_workspace,
            items::delete_workspace
        ))
        .routes(routes!(
            items::get_document_root,
            items::create_document_root,
            items::put_document_root,
            items::patch_document_root,
            items::delete_document_root
        ))
        .routes(routes!(
            items::get_document_workspace,
            items::create_document_workspace,
            items::put_document_workspace,
            items::patch_document_workspace,
            items::delete_document_workspace
        ))
        .routes(routes!(items::health))
        .routes(routes!(items::info))
        .split_for_parts();

    let api_json = serde_json::to_value(&api).expect("failed to serialize openapi");
    let api_json = Arc::new(api_json);

    Router::new()
        .merge(router)
        .route(
            "/openapi.json",
            get({
                let api_json = api_json.clone();
                move || async move { Json(api_json.as_ref().clone()) }
            }),
        )
        .layer(Extension(state))
        .layer(middleware::from_fn(logging::log_request_response))
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

    OpenApiBuilder::new()
        .info(info)
        .paths(Paths::new())
        .build()
}

fn cargo_contact() -> Option<utoipa::openapi::Contact> {
    let (name, email) = parse_author(env!("CARGO_PKG_AUTHORS"));
    if name.is_none() && email.is_none() {
        return None;
    }

    Some(
        ContactBuilder::new()
            .name(name)
            .email(email)
            .build(),
    )
}

fn parse_author(authors: &str) -> (Option<String>, Option<String>) {
    let first = authors
        .split(':')
        .next()
        .unwrap_or_default()
        .trim();
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
