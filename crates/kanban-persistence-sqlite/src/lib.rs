pub mod backend_factory;
pub mod sqlite_backend;
pub mod sqlite_store;

pub use backend_factory::SqliteBackendFactory;
pub use sqlite_backend::SqliteBackend;

pub use sqlite_store::SqliteStore;
pub use sqlite_store::SUPPORTED_SCHEMA_VERSION;

use kanban_domain::KanbanError;
use kanban_persistence::{PersistenceError, PersistenceStore, StoreFactory};
use std::sync::Arc;

/// Construct a SQLite database file at `path` with a `metadata` row whose
/// `schema_version` is forced to `version`. Intended for cross-crate
/// integration tests that need to exercise the `UnsupportedFutureVersion`
/// refusal at every surface (service, MCP, CLI) without each test
/// reimplementing the seed SQL.
///
/// Bypasses `SqliteStore::open` deliberately — opening would normalise the
/// version via `migrate()` before the test could observe the pre-bumped
/// state. Writes only the `metadata` table; the rest of the schema is
/// created by `SqliteStore::open` on first real load.
///
/// Gated behind the `test-helpers` feature so it does not ship in release
/// binaries. Mirrors the pattern in `kanban-persistence::test_helpers`.
#[cfg(feature = "test-helpers")]
pub async fn write_test_metadata_with_schema_version(
    path: &std::path::Path,
    version: u32,
) -> Result<(), PersistenceError> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
        .map_err(|e| PersistenceError::Database(e.to_string()))?;
    sqlx::raw_sql(&format!(
        "CREATE TABLE IF NOT EXISTS metadata (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            instance_id TEXT NOT NULL,
            saved_at TEXT NOT NULL,
            schema_version INTEGER NOT NULL DEFAULT {SUPPORTED_SCHEMA_VERSION},
            writer_version TEXT,
            writer_commit TEXT
        );
        INSERT OR REPLACE INTO metadata (id, instance_id, saved_at, schema_version)
        VALUES (1, '550e8400-e29b-41d4-a716-446655440000', '2030-01-01T00:00:00Z', {version});"
    ))
    .execute(&pool)
    .await
    .map_err(|e| PersistenceError::Database(e.to_string()))?;
    pool.close().await;
    Ok(())
}

/// Test-only companion to [`write_test_metadata_with_schema_version`]: probe
/// the metadata row's `schema_version` without going through `SqliteStore::open`
/// (which would normalise it). Used by integration tests that want to assert
/// a refused open didn't bump the on-disk version.
///
/// Gated behind the `test-helpers` feature; see the sibling function for
/// rationale.
#[cfg(feature = "test-helpers")]
pub async fn read_test_schema_version(
    path: &std::path::Path,
) -> Result<Option<u32>, PersistenceError> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(path))
        .await
        .map_err(|e| PersistenceError::Database(e.to_string()))?;
    let version: Option<u32> =
        sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
            .fetch_optional(&pool)
            .await
            .map_err(|e| PersistenceError::Database(e.to_string()))?;
    pool.close().await;
    Ok(version)
}

pub struct SqliteStoreFactory;

impl StoreFactory for SqliteStoreFactory {
    fn name(&self) -> &str {
        "sqlite"
    }

    fn matches_content(&self, header: &[u8]) -> bool {
        header.starts_with(b"SQLite format 3\0")
    }

    fn create(
        &self,
        locator: &str,
    ) -> Result<Arc<dyn PersistenceStore + Send + Sync>, PersistenceError> {
        let handle = tokio::runtime::Handle::current();
        if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::CurrentThread {
            return Err(PersistenceError::Database(
                "SqliteStoreFactory::create requires a multi-thread Tokio runtime; \
                 block_in_place is unavailable on a current_thread runtime. \
                 Use #[tokio::test(flavor = \"multi_thread\")] in tests."
                    .to_string(),
            ));
        }
        let store = tokio::task::block_in_place(|| handle.block_on(SqliteStore::open(locator)))
            // Preserve typed variants across the KanbanError → PersistenceError
            // boundary so downstream callers can discriminate them (esp.
            // UnsupportedFutureVersion via `KanbanError::is_unsupported_future_version`).
            // Stringifying everything into Database would flatten the typed
            // variant and break the cross-surface refusal contract.
            .map_err(|e| match e {
                KanbanError::UnsupportedFutureVersion {
                    file_version,
                    binary_max,
                } => PersistenceError::UnsupportedFutureVersion {
                    file_version,
                    binary_max,
                },
                other => PersistenceError::Database(other.to_string()),
            })?;
        Ok(Arc::new(store))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_persistence::StoreFactory;

    #[test]
    fn test_sqlite_factory_matches_content_sqlite_magic_bytes() {
        let header = b"SQLite format 3\0extra";
        assert!(SqliteStoreFactory.matches_content(header));
    }

    #[test]
    fn test_sqlite_factory_matches_content_rejects_json() {
        let header = b"{\"boards\": []}";
        assert!(!SqliteStoreFactory.matches_content(header));
    }

    #[test]
    fn test_sqlite_factory_matches_content_rejects_empty() {
        assert!(!SqliteStoreFactory.matches_content(b""));
    }

    #[test]
    fn test_sqlite_factory_name_is_sqlite() {
        assert_eq!(SqliteStoreFactory.name(), "sqlite");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sqlite_factory_create_returns_persistence_store() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let store = SqliteStoreFactory.create(path.to_str().unwrap()).unwrap();
        assert!(store.exists().await);
    }
}
