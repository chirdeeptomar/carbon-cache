// application/src/lib.rs
pub mod ports;
pub mod usecases;

use ports::CacheStore;
use std::sync::Arc;
use usecases::{DeleteUseCase, GetUseCase, PutUseCase};

#[derive(Clone)]
pub struct Application {
    pub put: PutUseCase,
    pub get: GetUseCase,
    pub delete: DeleteUseCase,
}

impl Application {
    pub fn new(store: Arc<dyn CacheStore>) -> Self {
        Self {
            put: PutUseCase::new(store.clone()),
            get: GetUseCase::new(store.clone()),
            delete: DeleteUseCase::new(store),
        }
    }
}
