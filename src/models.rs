pub const DEFAULT_FIELDS: [&str; 5] = ["name", "title", "label", "description", "category"];

#[derive(Debug, Default, Clone)]
pub struct ListQuery {
    pub term: Option<String>,
    pub filter: Vec<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}
