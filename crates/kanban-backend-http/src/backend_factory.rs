use crate::HttpBackend;
use kanban_backend::{KanbanBackend, KanbanBackendFactory};
use kanban_core::AppConfig;
use kanban_domain::KanbanResult;
use std::sync::Arc;

pub struct HttpBackendFactory;

#[async_trait::async_trait]
impl KanbanBackendFactory for HttpBackendFactory {
    fn name(&self) -> &str {
        "http"
    }

    fn matches_locator(&self, locator: &str, _header: &[u8]) -> bool {
        matches!(kanban_core::scheme_of(locator), Some("http" | "https"))
    }

    fn is_remote(&self) -> bool {
        true
    }

    async fn create(
        &self,
        locator: &str,
        _config: &AppConfig,
    ) -> KanbanResult<Arc<dyn KanbanBackend>> {
        Ok(Arc::new(HttpBackend::new(locator)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_backend::KanbanBackendFactory;

    #[test]
    fn test_the_http_factory_claims_an_http_locator() {
        assert_eq!(HttpBackendFactory.name(), "http");
        assert!(HttpBackendFactory.matches_locator("http://127.0.0.1:3000", &[]));
        assert!(HttpBackendFactory.matches_locator("https://example.com", &[]));
    }

    #[test]
    fn test_the_http_factory_declines_a_local_path_and_a_foreign_scheme() {
        assert!(!HttpBackendFactory.matches_locator("board.json", &[]));
        assert!(!HttpBackendFactory.matches_locator("/abs/board.sqlite", &[]));
        assert!(!HttpBackendFactory.matches_locator("C://boards", &[]));
        assert!(!HttpBackendFactory.matches_locator("notes://draft.json", &[]));
    }

    #[test]
    fn test_the_http_factory_declares_itself_remote() {
        assert!(HttpBackendFactory.is_remote());
    }
}
