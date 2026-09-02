use crate::config;
use crate::AppConfig;
use kanban_domain::{DataStore, KanbanError};
use kanban_persistence::{snapshot_from_json_bytes, PersistenceStore, StoreRegistry};
use std::sync::Arc;

/// Owns the `StoreRegistry` and `KanbanBackendRegistry` and exposes the
/// high-level operations that used to live as free functions in
/// `kanban_service`. Callers (the CLI, TUI, MCP) construct a `StoreManager`
/// with whichever factories they want available, then thread it through
/// request handlers — inverting the old model where `kanban-service`
/// hard-coded `default_registry()`.
pub struct StoreManager {
    registry: Arc<StoreRegistry>,
    backends: Arc<kanban_backend::KanbanBackendRegistry>,
}

impl StoreManager {
    /// Wraps `registry` and `backends` in an `Arc` each. Cloning a
    /// `StoreManager` is cheap — all clones share the same underlying
    /// registries.
    pub fn new(registry: StoreRegistry, backends: kanban_backend::KanbanBackendRegistry) -> Self {
        Self {
            registry: Arc::new(registry),
            backends: Arc::new(backends),
        }
    }

    /// Returns a reference to the underlying `StoreRegistry`.
    /// Useful for introspection and testing.
    pub fn registry(&self) -> &StoreRegistry {
        &self.registry
    }

    /// Returns `true` if at least one backend factory is registered.
    pub fn has_backends(&self) -> bool {
        !self.backends.is_empty()
    }

    /// Returns the names of all registered backend factories in
    /// registration order.
    pub fn backend_names(&self) -> Vec<&str> {
        self.backends.names()
    }

    /// Returns `true` if `locator` points to a SQLite database — either
    /// because `detect_backend` recognised it as `"sqlite"`, or because the
    /// file extension matches one of the conventional SQLite extensions.
    pub fn is_sqlite(&self, locator: &str) -> bool {
        match self.detect_backend(locator).as_deref() {
            Some("sqlite") => true,
            None => {
                locator.ends_with(".sqlite")
                    || locator.ends_with(".sqlite3")
                    || locator.ends_with(".db")
            }
            _ => false,
        }
    }

    /// Pattern-matches `locator` against all registered factories and returns
    /// the name of the first match. Falls back to the backend registry when
    /// no store factory matches.
    pub fn detect_backend(&self, locator: &str) -> Option<String> {
        if let Some(name) = self.registry.detect_backend(locator) {
            return Some(name.to_string());
        }
        self.backends
            .for_locator(locator)
            .map(|f| f.name().to_string())
    }

    /// Updates `config.storage_backend` to match the backend inferred from
    /// `locator`. Returns `true` if the config value changed.
    pub fn sync_backend_with_file(&self, locator: &str, config: &mut AppConfig) -> bool {
        if let Some(detected) = self.detect_backend(locator) {
            if detected != config.effective_storage_backend() {
                config.storage_backend = Some(detected);
                return true;
            }
        }
        false
    }

    /// Creates a [`KanbanBackend`] for `locator` by dispatching through the
    /// injected `KanbanBackendRegistry` — the first registered factory whose
    /// `matches_locator` accepts `locator` builds it.
    pub async fn make_backend(
        &self,
        locator: &str,
        config: &AppConfig,
    ) -> Result<std::sync::Arc<dyn crate::backend::KanbanBackend>, KanbanError> {
        let factory = self.backends.for_locator(locator).ok_or_else(|| {
            KanbanError::Internal(format!(
                "no registered backend handles '{locator}'; registered: {:?}",
                self.backends.names()
            ))
        })?;
        factory.create(locator, config).await
    }

    /// Creates a [`KanbanBackend`] for `locator` by exact backend name,
    /// bypassing header/extension sniffing entirely.
    pub async fn make_backend_named(
        &self,
        backend: &str,
        locator: &str,
        config: &AppConfig,
    ) -> Result<std::sync::Arc<dyn crate::backend::KanbanBackend>, KanbanError> {
        let factory = self.backends.for_name(backend).ok_or_else(|| {
            KanbanError::Internal(format!(
                "no registered backend named '{backend}'; registered: {:?}",
                self.backends.names()
            ))
        })?;
        factory.create(locator, config).await
    }

    /// Creates a `PersistenceStore` for the named `backend` at `locator`.
    /// Returns an error if `backend` is not registered in this manager.
    pub fn make_store(
        &self,
        backend: &str,
        locator: &str,
    ) -> Result<Arc<dyn PersistenceStore + Send + Sync>, KanbanError> {
        Ok(self.registry.create_store(backend, locator)?)
    }

    /// Creates a store from an explicit file locator, or falls back to the
    /// storage location in `config` when `file` is `None`. The backend is
    /// inferred from the locator; if no factory matches, `config`'s backend
    /// is used as a fallback.
    pub fn make_store_with_config(
        &self,
        file: Option<&str>,
        config: &AppConfig,
    ) -> Result<Arc<dyn PersistenceStore + Send + Sync>, KanbanError> {
        let locator = match file {
            Some(path) => path.to_string(),
            None => config::resolve_storage_location(config),
        };
        let backend = self
            .detect_backend(&locator)
            .unwrap_or_else(|| config.effective_storage_backend().to_string());
        self.make_store(&backend, &locator)
    }

    /// Verifies `path` is a readable, valid store for `backend` without
    /// returning its contents.
    ///
    /// For `.sqlite`/`.db` files, opens through `SqliteBackend` and issues a
    /// cheap real read (`list_boards`) that proves the store is openable and
    /// queryable, rather than deserialising the whole store: opening already
    /// runs schema migration and fails loudly on a corrupt or future-version
    /// file (side effects — see the comment below — are pre-existing and
    /// intentional), so a full table scan on top of that buys nothing.
    /// Non-SQLite backends keep a full parse: for JSON the envelope IS the
    /// file, so "readable" means "the whole file parses"; a partial read
    /// could miss corruption in an untouched region.
    pub async fn validate_store_readable(
        &self,
        backend: &str,
        path: &str,
    ) -> Result<(), KanbanError> {
        if matches!(backend, "sqlite" | "sqlite3" | "db") {
            #[cfg(feature = "sqlite")]
            {
                if !std::path::Path::new(path).exists() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("Storage file does not exist: {}", path),
                    )
                    .into());
                }
                // Opening runs schema migration and writes a `.v{N}.backup`;
                // pre-existing and intentional — a validate that cannot open
                // the file is not a validate.
                let sqlite_backend = kanban_persistence_sqlite::SqliteBackend::open(path).await?;
                sqlite_backend.list_boards()?;
                return Ok(());
            }
            #[cfg(not(feature = "sqlite"))]
            return Err(KanbanError::validation("sqlite feature not compiled in"));
        }
        let store = self.make_store(backend, path)?;
        if !store.exists().await {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Storage file does not exist: {}", path),
            )
            .into());
        }
        let (snapshot, _metadata) = store.load().await?;
        snapshot_from_json_bytes(&snapshot.data)?;
        Ok(())
    }

    /// Exports a board selection to a new SQLite file.
    ///
    /// **Note:** The dependency graph is not part of the `AllBoardsExport` format
    /// and will not be present in the exported file.
    pub async fn export_to_sqlite(
        &self,
        export: kanban_domain::export::AllBoardsExport,
        filename: &str,
        config: &AppConfig,
    ) -> Result<(), KanbanError> {
        #[cfg(feature = "sqlite")]
        {
            use kanban_domain::export::BoardImporter;
            use kanban_domain::{DependencyGraph, Snapshot};

            // Mirrors migrate_store. The path this replaced wiped the
            // destination's tables before inserting, so exporting onto an
            // existing database silently replaced it; writing per entity would
            // instead merge the export on top of whatever was there. Refusing
            // an existing file makes the caller choose rather than either.
            if std::path::Path::new(filename).exists() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!(
                        "Destination already exists: {filename}. Remove it first or use a different path."
                    ),
                )
                .into());
            }

            let mut entities = BoardImporter::extract_entities(export);
            entities.cards = entities
                .cards
                .iter()
                .map(|c| {
                    kanban_domain::stamp_card_prefix(
                        c,
                        &entities.columns,
                        &entities.boards,
                        &entities.sprints,
                    )
                })
                .collect();
            // `AllBoardsExport` carries no counters, so they are reconstructed
            // from the entities that consumed them. Without this the exported
            // database hands out numbers its own cards already hold.
            let prefixes = kanban_domain::counters_implied_by(
                &entities.cards,
                &entities.columns,
                &entities.sprints,
                &entities.boards,
                Some(config.effective_default_sprint_prefix()),
            );
            let snapshot = Snapshot {
                archived_boards: entities.archived_boards,
                boards: entities.boards,
                columns: entities.columns,
                cards: entities.cards,
                archived_cards: entities.archived_cards,
                sprints: entities.sprints,
                graph: DependencyGraph::default(),
                prefixes,
            };
            self.write_sqlite_destination(filename, snapshot).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = export;
            let _ = filename;
            Err(KanbanError::validation("sqlite feature not compiled in"))
        }
    }

    /// Copies a snapshot from one backend/path pair to another, repairing
    /// any dangling foreign keys in the process. Rolls back (deletes the
    /// partial destination file) on failure.
    ///
    /// Both legs produce and consume a typed `kanban_domain::Snapshot`.
    /// SQLite ends are handled directly via `SqliteBackend`; JSON and other
    /// registry-backed ends go through the `KanbanBackendRegistry`
    /// (`make_backend` for the source, whose header is authoritative, and
    /// `make_backend_named` for the destination, which does not exist yet).
    pub async fn migrate_store(
        &self,
        from_backend: &str,
        from_path: &str,
        to_backend: &str,
        to_path: &str,
    ) -> Result<(), KanbanError> {
        let from = std::path::Path::new(from_path);
        let to = std::path::Path::new(to_path);
        if !from.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Source file not found: {}", from.display()),
            )
            .into());
        }
        if to.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "Destination already exists: {}. Remove it first or use a different path.",
                    to.display()
                ),
            )
            .into());
        }

        let leg = MigrationLeg::of(from_backend, to_backend);
        let source_is_sqlite = matches!(
            leg,
            MigrationLeg::SqliteToSqlite | MigrationLeg::SqliteToJson
        );
        let dest_is_sqlite = matches!(
            leg,
            MigrationLeg::SqliteToSqlite | MigrationLeg::JsonToSqlite
        );

        // SQLite -> SQLite never touches JSON: atomic reads feed transactional
        // writes directly. Foreign keys are checked as each row lands, so
        // write_full_snapshot's ordering carries the correctness here, not the
        // transaction.
        if !leg.round_trips_through_json() {
            #[cfg(feature = "sqlite")]
            {
                let snapshot = self.read_sqlite_source(from_path).await?;
                return self.write_sqlite_destination(to_path, snapshot).await;
            }
            #[cfg(not(feature = "sqlite"))]
            return Err(KanbanError::validation("sqlite feature not compiled in"));
        }

        // Every other leg has JSON on one end, so `Snapshot`-level FK repair
        // applies to it.
        let mut snapshot: kanban_domain::Snapshot = if source_is_sqlite {
            #[cfg(feature = "sqlite")]
            {
                self.read_sqlite_source(from_path).await?
            }
            #[cfg(not(feature = "sqlite"))]
            return Err(KanbanError::validation("sqlite feature not compiled in"));
        } else {
            let backend = self.make_backend(from_path, &AppConfig::default()).await?;
            let snap = crate::store_adapter::read_full_snapshot(backend.as_data_store());
            drop(backend);
            snap?
        };

        crate::store_adapter::repair_fks(&mut snapshot)?;

        if dest_is_sqlite {
            #[cfg(feature = "sqlite")]
            {
                return self.write_sqlite_destination(to_path, snapshot).await;
            }
            #[cfg(not(feature = "sqlite"))]
            return Err(KanbanError::validation("sqlite feature not compiled in"));
        }

        self.write_registry_destination(to_backend, to_path, snapshot)
            .await
    }

    /// Reads a SQLite source into a `Snapshot` through per-entity `DataStore`
    /// calls.
    ///
    /// Opening a source below `SUPPORTED_SCHEMA_VERSION` runs the same in-place
    /// schema upgrade (and durable `.v{N}.backup`) that any kanban binary runs
    /// against that file — reading a schema-current workspace requires it.
    #[cfg(feature = "sqlite")]
    async fn read_sqlite_source(
        &self,
        from_path: &str,
    ) -> Result<kanban_domain::Snapshot, KanbanError> {
        use kanban_backend::KanbanBackend;

        let backend = kanban_persistence_sqlite::SqliteBackend::open(from_path).await?;
        let snapshot = crate::store_adapter::read_full_snapshot(backend.as_data_store());
        backend.close().await;
        drop(backend);
        snapshot
    }

    /// Writes a whole workspace into a SQLite destination inside one
    /// transaction, cleaning up on failure.
    ///
    /// Cleanup only removes a file this call brought into existence. `migrate_store`
    /// rejects an existing destination up front, so its behaviour is unchanged;
    /// `export_to_sqlite` has no such guard, and deleting a database the caller
    /// already had would turn a failed export into data loss.
    ///
    /// `close()` runs before the cleanup because Windows refuses to unlink a
    /// file that still has live handles.
    #[cfg(feature = "sqlite")]
    async fn write_sqlite_destination(
        &self,
        to_path: &str,
        snapshot: kanban_domain::Snapshot,
    ) -> Result<(), KanbanError> {
        use kanban_backend::KanbanBackend;

        let created_here = !std::path::Path::new(to_path).exists();

        let backend = kanban_persistence_sqlite::SqliteBackend::open(to_path).await?;
        let outcome = backend.with_transaction(Box::new(|| {
            crate::store_adapter::write_full_snapshot(backend.as_data_store(), snapshot)
        }));
        backend.close().await;
        drop(backend);
        if let Err(e) = outcome {
            if created_here {
                cleanup_destination_files(to_path).await;
            }
            return Err(e);
        }
        Ok(())
    }

    /// Writes a whole workspace into a registry-backed (non-SQLite)
    /// destination via `make_backend_named`, flushing before drop since these
    /// backends may buffer writes in memory until told to persist.
    async fn write_registry_destination(
        &self,
        to_backend: &str,
        to_path: &str,
        snapshot: kanban_domain::Snapshot,
    ) -> Result<(), KanbanError> {
        let created_here = !std::path::Path::new(to_path).exists();

        let backend = self
            .make_backend_named(to_backend, to_path, &AppConfig::default())
            .await?;
        let outcome = match backend.with_transaction(Box::new(|| {
            crate::store_adapter::write_full_snapshot(backend.as_data_store(), snapshot)
        })) {
            Ok(()) => backend.flush().await,
            Err(e) => Err(e),
        };
        drop(backend);
        if let Err(e) = outcome {
            if created_here {
                cleanup_destination_files(to_path).await;
            }
            return Err(e);
        }
        Ok(())
    }
}

impl Clone for StoreManager {
    fn clone(&self) -> Self {
        Self {
            registry: Arc::clone(&self.registry),
            backends: Arc::clone(&self.backends),
        }
    }
}

/// Best-effort `remove_file` that retries with linear backoff. SQLite on
/// Windows can briefly hold a file handle even after `PersistenceStore::close`
/// returns, because the OS-level handle release is asynchronous and lags the
/// pool's synchronization. POSIX always succeeds on the first try.
async fn remove_file_with_windows_retry(path: &std::path::Path) {
    for delay_ms in [0u64, 50, 100, 200, 400] {
        if delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
        if std::fs::remove_file(path).is_ok() {
            return;
        }
        if !path.exists() {
            return;
        }
    }
    tracing::warn!(
        path = %path.display(),
        "failed to remove file after retry backoff; orphan may remain on disk"
    );
}

/// Remove the main destination and its SQLite WAL/SHM sidecars (best-effort).
/// The sidecars are named `<path>-wal` and `<path>-shm` regardless of the
/// main file's extension, so they're constructed by appending to the raw
/// path string rather than via `Path::with_extension`.
fn is_sqlite_backend(name: &str) -> bool {
    matches!(name, "sqlite" | "sqlite3" | "db")
}

/// Which of the four source/destination combinations a migration takes.
/// `SqliteToSqlite` is the one leg that carries no JSON at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MigrationLeg {
    SqliteToSqlite,
    SqliteToJson,
    JsonToSqlite,
    JsonToJson,
}

impl MigrationLeg {
    pub(crate) fn of(from_backend: &str, to_backend: &str) -> Self {
        match (
            is_sqlite_backend(from_backend),
            is_sqlite_backend(to_backend),
        ) {
            (true, true) => Self::SqliteToSqlite,
            (true, false) => Self::SqliteToJson,
            (false, true) => Self::JsonToSqlite,
            (false, false) => Self::JsonToJson,
        }
    }

    /// Whether this leg serialises through JSON bytes. Only `SqliteToSqlite`
    /// does not, and that is therefore the one leg FK repair cannot run on. It
    /// does not need it: its source is a relational database that already
    /// enforced those keys on the way in.
    pub(crate) fn round_trips_through_json(self) -> bool {
        !matches!(self, Self::SqliteToSqlite)
    }
}

async fn cleanup_destination_files(to_path: &str) {
    remove_file_with_windows_retry(std::path::Path::new(to_path)).await;
    let wal = format!("{}-wal", to_path);
    let shm = format!("{}-shm", to_path);
    remove_file_with_windows_retry(std::path::Path::new(&wal)).await;
    remove_file_with_windows_retry(std::path::Path::new(&shm)).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_persistence::StoreRegistry;
    use tempfile::tempdir;

    #[test]
    fn test_sqlite_to_sqlite_leg_does_not_round_trip_through_json() {
        // The whole point of the adapter: a SQLite-to-SQLite move reads
        // atomically straight into transactional writes, with no intermediate
        // serialisation. Asserted on the routing decision so it cannot regress
        // into a JSON round trip unnoticed.
        for (from, to) in [("sqlite", "sqlite"), ("sqlite3", "db"), ("db", "sqlite3")] {
            let leg = MigrationLeg::of(from, to);
            assert_eq!(leg, MigrationLeg::SqliteToSqlite, "{from} -> {to}");
            assert!(
                !leg.round_trips_through_json(),
                "{from} -> {to} must not serialise through JSON"
            );
        }
    }

    #[test]
    fn test_every_leg_with_json_on_one_end_round_trips_through_json() {
        // FK repair operates on JSON bytes, so it can only run where they exist.
        for (from, to, expected) in [
            ("sqlite", "json", MigrationLeg::SqliteToJson),
            ("json", "sqlite", MigrationLeg::JsonToSqlite),
            ("json", "json", MigrationLeg::JsonToJson),
        ] {
            let leg = MigrationLeg::of(from, to);
            assert_eq!(leg, expected, "{from} -> {to}");
            assert!(
                leg.round_trips_through_json(),
                "{from} -> {to} carries a StoreSnapshot, so FK repair applies"
            );
        }
    }

    fn make_sm() -> StoreManager {
        let mut registry = StoreRegistry::new();
        registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
        let mut backends = kanban_backend::KanbanBackendRegistry::new();
        #[cfg(feature = "sqlite")]
        backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
        backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
        StoreManager::new(registry, backends)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_make_backend_errors_for_unregistered_locator() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.json");
        let sm = StoreManager::new(
            StoreRegistry::new(),
            kanban_backend::KanbanBackendRegistry::new(),
        );
        let cfg = AppConfig::default();
        let err = sm
            .make_backend(path.to_str().unwrap(), &cfg)
            .await
            .err()
            .expect("empty registry must not silently pick a backend");
        let msg = err.to_string();
        assert!(
            msg.contains("no registered backend handles"),
            "expected unregistered-locator error, got: {msg}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_make_backend_json_path_returns_json_data_store() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.json");
        let sm = make_sm();
        let cfg = AppConfig::default();
        let backend = sm.make_backend(path.to_str().unwrap(), &cfg).await.unwrap();
        assert!(!backend.needs_flush(), "new JSON backend starts clean");
        assert!(
            backend.needs_save_worker(),
            "JSON backend requires a background flush worker"
        );
    }

    #[cfg(feature = "sqlite")]
    mod sqlite_backend_tests {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn test_make_backend_sqlite_path_returns_sqlite_store() {
            let dir = tempdir().unwrap();
            let path = dir.path().join("test.sqlite");
            let sm = make_sm();
            let cfg = AppConfig::default();
            let backend = sm.make_backend(path.to_str().unwrap(), &cfg).await.unwrap();
            assert!(!backend.needs_flush(), "new SQLite backend starts clean");
            assert!(
                !backend.needs_save_worker(),
                "SQLite backend is write-through and does not need a save worker"
            );
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn test_make_backend_detects_sqlite_by_magic_bytes() {
            let dir = tempdir().unwrap();
            let path = dir.path().join("noext");

            // Create a real SQLite file with no extension so the registry can
            // detect it via magic bytes.
            kanban_persistence_sqlite::SqliteStore::open(path.to_str().unwrap())
                .await
                .unwrap();

            let sm = make_sm();
            let cfg = AppConfig::default();
            let backend = sm.make_backend(path.to_str().unwrap(), &cfg).await.unwrap();
            assert!(
                !backend.needs_save_worker(),
                "magic-byte SQLite detection should yield a write-through backend"
            );
            let boards = backend.list_boards().unwrap();
            assert!(boards.is_empty());
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn test_make_backend_detects_json_by_content() {
            use kanban_persistence::{PersistenceMetadata, PersistenceStore, StoreSnapshot};
            let dir = tempdir().unwrap();
            let path = dir.path().join("noext");

            // Write a valid JSON envelope file with no extension so the registry
            // detects it as JSON via content sniffing (first byte is '{').
            {
                let jfs = kanban_persistence_json::JsonFileStore::new(&path);
                let snap = kanban_domain::Snapshot::new();
                let data = kanban_persistence::snapshot_to_json_bytes(&snap).unwrap();
                let meta = PersistenceMetadata::new(uuid::Uuid::new_v4());
                jfs.save(StoreSnapshot {
                    data,
                    metadata: meta,
                })
                .await
                .unwrap();
            }

            let sm = make_sm();
            let cfg = AppConfig::default();
            let backend = sm.make_backend(path.to_str().unwrap(), &cfg).await.unwrap();
            assert!(
                backend.needs_save_worker(),
                "content-sniffed JSON backend requires a save worker"
            );
            let boards = backend.list_boards().unwrap();
            assert!(boards.is_empty());
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn test_sqlite_locator_resolves_without_a_store_factory() {
            let mut registry = StoreRegistry::new();
            registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
            let mut backends = kanban_backend::KanbanBackendRegistry::new();
            backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
            backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
            let sm = StoreManager::new(registry, backends);

            let dir = tempdir().unwrap();
            let path = dir.path().join("test.db");
            let cfg = AppConfig::default();
            let backend = sm
                .make_backend(path.to_str().unwrap(), &cfg)
                .await
                .expect("make_backend must resolve a sqlite locator without a store-level factory registered");
            let boards = backend.list_boards().unwrap();
            assert!(boards.is_empty());
        }

        #[test]
        fn test_detect_backend_on_a_nonexistent_db_path_returns_sqlite() {
            let sm = make_sm();
            let dir = tempdir().unwrap();
            let path = dir.path().join("does_not_exist.db");
            assert_eq!(
                sm.detect_backend(path.to_str().unwrap()),
                Some("sqlite".to_string())
            );
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn test_detect_backend_on_an_existing_sqlite_file_still_returns_sqlite() {
            let dir = tempdir().unwrap();
            let path = dir.path().join("existing.db");
            kanban_persistence_sqlite::SqliteStore::open(path.to_str().unwrap())
                .await
                .unwrap();

            let sm = make_sm();
            assert_eq!(
                sm.detect_backend(path.to_str().unwrap()),
                Some("sqlite".to_string())
            );
        }

        #[test]
        fn test_detect_backend_with_no_registered_factories_returns_none() {
            let sm = StoreManager::new(
                StoreRegistry::new(),
                kanban_backend::KanbanBackendRegistry::new(),
            );

            assert_eq!(sm.detect_backend("whatever.db"), None);
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn test_make_backend_named_ignores_the_locator_header() {
            let dir = tempdir().unwrap();
            let path = dir.path().join("dest.db");
            let sm = make_sm();
            let cfg = AppConfig::default();

            let named = sm
                .make_backend_named("json", path.to_str().unwrap(), &cfg)
                .await
                .unwrap();
            assert!(
                named.needs_save_worker(),
                "make_backend_named(\"json\", ...) must build a JSON backend regardless of the .db extension"
            );

            let sniffed = sm.make_backend(path.to_str().unwrap(), &cfg).await.unwrap();
            assert!(
                !sniffed.needs_save_worker(),
                "make_backend on the same nonexistent .db path sniffs sqlite by extension"
            );
        }

        #[test]
        fn test_detect_backend_for_a_db_path_without_a_sqlite_backend_falls_back_to_json() {
            let mut registry = StoreRegistry::new();
            registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
            let mut backends = kanban_backend::KanbanBackendRegistry::new();
            backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
            let sm = StoreManager::new(registry, backends);

            assert_eq!(sm.detect_backend("whatever.db"), Some("json".to_string()));
        }
    }
}
