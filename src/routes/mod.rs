mod items;

use axum::Router;

use crate::services::AppState;

pub fn router(state: AppState) -> Router {
    items::router(state)
}
