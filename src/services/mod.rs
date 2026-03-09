pub mod contract;
pub mod store;

use std::sync::{Arc, RwLock};

pub use contract::ApiContract;
pub use store::Store;

pub const DEFAULT_WORKSPACE_NAME: &str = "default";

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub use_default_workspace: bool,
    api_contract: Arc<RwLock<Option<ApiContract>>>,
}

impl AppState {
    pub fn new(
        store: Store,
        use_default_workspace: bool,
        api_contract: Option<ApiContract>,
    ) -> Self {
        Self {
            store,
            use_default_workspace,
            api_contract: Arc::new(RwLock::new(api_contract)),
        }
    }

    pub fn api_contract(&self) -> Option<ApiContract> {
        self.api_contract
            .read()
            .expect("api contract lock poisoned")
            .clone()
    }

    pub fn replace_api_contract(&self, api_contract: Option<ApiContract>) {
        *self
            .api_contract
            .write()
            .expect("api contract lock poisoned") = api_contract;
    }
}
