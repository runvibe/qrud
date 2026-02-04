pub mod contract;
pub mod store;

pub use contract::ApiContract;
pub use store::Store;

pub const DEFAULT_WORKSPACE_NAME: &str = "default";

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub use_default_workspace: bool,
    pub api_contract: Option<ApiContract>,
}

impl AppState {
    pub fn new(store: Store, use_default_workspace: bool, api_contract: Option<ApiContract>) -> Self {
        Self {
            store,
            use_default_workspace,
            api_contract,
        }
    }
}
