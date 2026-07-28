use async_trait::async_trait;
use kanban_domain::data_store::DataStore;
use kanban_domain::{KanbanError, KanbanResult};
use uuid::Uuid;

pub mod client;
pub mod command_store;
pub mod data_store;
pub mod error;

use crate::backend::KanbanBackend;

/// HTTP client backend that connects to a kanban-server instance.
/// Implements the same `KanbanBackend` interface over HTTP, allowing a TUI/CLI
/// to work with shared boards on a remote server.
pub struct HttpBackend {
    base_url: String,
    client: reqwest::blocking::Client,
    instance_id: Uuid,
}

impl HttpBackend {
    /// Create a new HTTP backend connected to a kanban server at the given URL.
    /// The base URL is normalized (trailing slash removed).
    ///
    /// # Arguments
    /// * `base_url` - The base URL of the kanban server (e.g., "http://localhost:8080")
    ///
    /// # Errors
    /// Returns `KanbanError::Internal` if the HTTP client cannot be built.
    pub fn new(base_url: &str) -> KanbanResult<Self> {
        let base_url = base_url.trim_end_matches('/').to_string();
        let client = reqwest::blocking::Client::builder()
            .build()
            .map_err(|e| KanbanError::Internal(format!("failed to build http client: {e}")))?;
        Ok(Self {
            base_url,
            client,
            instance_id: Uuid::new_v4(),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn client(&self) -> &reqwest::blocking::Client {
        &self.client
    }
}

#[async_trait]
impl KanbanBackend for HttpBackend {
    fn as_data_store(&self) -> &dyn DataStore {
        self
    }

    fn instance_id(&self) -> Uuid {
        self.instance_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::KanbanBackend;

    #[test]
    fn test_http_backend_new_normalizes_trailing_slash() {
        let backend = HttpBackend::new("http://localhost:8080/").unwrap();
        assert_eq!(backend.base_url(), "http://localhost:8080");
    }

    #[test]
    fn test_http_backend_new_mints_nonnil_instance_id() {
        let backend = HttpBackend::new("http://localhost:8080").unwrap();
        assert_ne!(backend.instance_id, Uuid::nil());
    }

    #[test]
    fn test_http_backend_implements_kanban_backend_object_safe() {
        let backend = HttpBackend::new("http://localhost:8080").unwrap();
        let _: &dyn KanbanBackend = &backend;
    }

    #[test]
    fn test_http_backend_as_data_store_returns_self() {
        let backend = HttpBackend::new("http://localhost:8080").unwrap();
        let backend_ref: &dyn KanbanBackend = &backend;
        let _: &dyn DataStore = backend_ref.as_data_store();
    }

    #[test]
    fn test_http_backend_stub_method_returns_unsupported_error() {
        let backend = HttpBackend::new("http://localhost:8080").unwrap();
        let result = backend.list_boards();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_unsupported());
    }
}
