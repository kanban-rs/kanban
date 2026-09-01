pub mod backend_factory;
pub mod sqlite_backend;
pub mod sqlite_store;

pub use backend_factory::SqliteBackendFactory;
pub use sqlite_backend::SqliteBackend;

pub use sqlite_store::SqliteStore;
pub use sqlite_store::SUPPORTED_SCHEMA_VERSION;

#[cfg(feature = "test-helpers")]
use kanban_persistence::PersistenceError;

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
