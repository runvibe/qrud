use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const DEFAULT_FIELDS: [&str; 5] = ["name", "title", "label", "description", "category"];

#[derive(Debug, Default, Clone)]
pub struct ListQuery {
    pub term: Option<String>,
    pub filter: Vec<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(transparent)]
#[schema(value_type = Value)]
pub struct AnyJson(pub serde_json::Value);
