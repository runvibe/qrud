pub mod store;

pub use store::Store;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
}

impl AppState {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
}
