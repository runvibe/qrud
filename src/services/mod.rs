pub mod store;

pub use store::Store;

pub const DEFAULT_WORKSPACE_NAME: &str = "default";

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub use_default_workspace: bool,
}

impl AppState {
    pub fn new(store: Store, use_default_workspace: bool) -> Self {
        Self {
            store,
            use_default_workspace,
        }
    }
}
