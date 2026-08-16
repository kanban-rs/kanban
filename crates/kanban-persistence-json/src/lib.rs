pub mod atomic_writer;
pub mod backend_factory;
pub mod conflict;
pub mod json_backend;
pub mod json_file_store;
pub mod migration;
pub mod migration_test_support;

pub use backend_factory::JsonBackendFactory;
pub use conflict::FileMetadata;
pub use json_backend::JsonDataStore;
pub use json_file_store::{JsonEnvelope, JsonFileStore};

use kanban_persistence::{PersistenceError, PersistenceStore, StoreFactory};
use std::sync::Arc;

pub struct JsonStoreFactory;

impl StoreFactory for JsonStoreFactory {
    fn name(&self) -> &str {
        "json"
    }

    fn matches_content(&self, header: &[u8]) -> bool {
        let mut iter = header.iter();
        // Skip UTF-8 BOM if present
        if header.starts_with(&[0xEF, 0xBB, 0xBF]) {
            iter = header[3..].iter();
        }
        // Skip whitespace, check for JSON start
        let first = iter.find(|b| !b.is_ascii_whitespace());
        matches!(first, Some(b'{') | Some(b'['))
    }

    fn create(
        &self,
        locator: &str,
    ) -> Result<Arc<dyn PersistenceStore + Send + Sync>, PersistenceError> {
        Ok(Arc::new(JsonFileStore::new(locator)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_content_json_object() {
        let factory = JsonStoreFactory;
        assert!(factory.matches_content(b"{\"boards\": []}"));
    }

    #[test]
    fn test_matches_content_json_array() {
        let factory = JsonStoreFactory;
        assert!(factory.matches_content(b"[1, 2, 3]"));
    }

    #[test]
    fn test_matches_content_json_with_bom() {
        let factory = JsonStoreFactory;
        let mut data = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        data.extend_from_slice(b"  {\"key\": true}");
        assert!(factory.matches_content(&data));
    }

    #[test]
    fn test_matches_content_not_json() {
        let factory = JsonStoreFactory;
        assert!(!factory.matches_content(b"SQLite format 3\0"));
        assert!(!factory.matches_content(b""));
        assert!(!factory.matches_content(b"plain text"));
        assert!(!factory.matches_content(b"   \t\n  "));
    }
}
