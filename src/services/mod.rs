use std::sync::{Arc, Mutex};

pub mod store;

pub use store::Store;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Mutex<Store>>,
}

impl AppState {
    pub fn new(store: Store) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
        }
    }
}

pub fn lock_store(state: &AppState) -> std::sync::MutexGuard<'_, Store> {
    state.store.lock().expect("store lock poisoned")
}
