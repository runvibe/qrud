mod items;

use std::sync::Arc;

use axum::routing::get;
use axum::{Json, Router};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::services::AppState;

pub fn router(state: AppState) -> Router {
    let (router, api) = OpenApiRouter::new()
        .routes(routes!(items::list_collection, items::create_item))
        .routes(routes!(
            items::get_item,
            items::put_item,
            items::patch_item,
            items::delete_item
        ))
        .with_state(state)
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
}
