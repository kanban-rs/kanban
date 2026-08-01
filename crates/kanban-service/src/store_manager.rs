use crate::config;
use crate::AppConfig;
use kanban_domain::{DataStore, KanbanError};
use kanban_persistence::{
    snapshot_from_json_bytes, PersistenceStore, StoreRegistry, StoreSnapshot,
};
use std::collections::HashSet;
use std::sync::Arc;

/// Owns the `StoreRegistry` and exposes the high-level operations that used
/// to live as free functions in `kanban_service`. Callers (the CLI, TUI, MCP)
/// construct a `StoreManager` with whichever factories they want available,
/// then thread it through request handlers — inverting the old model where
/// `kanban-service` hard-coded `default_registry()`.
pub struct StoreManager {
    registry: Arc<StoreRegistry>,
}

impl StoreManager {
    /// Wraps `registry` in an `Arc`. Cloning a `StoreManager` is cheap —
    /// all clones share the same underlying registry.
    pub fn new(registry: StoreRegistry) -> Self {
        Self {
            registry: Arc::new(registry),
        }
    }

    /// Returns a reference to the underlying `StoreRegistry`.
    /// Useful for introspection and testing.
    pub fn registry(&self) -> &StoreRegistry {
        &self.registry
    }

    /// Returns `true` if at least one backend factory is registered.
    pub fn has_backends(&self) -> bool {
        !self.registry.is_empty()
    }

    /// Returns the names of all registered factories in registration order.
    pub fn backend_names(&self) -> Vec<&str> {
        self.registry.backend_names()
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
    /// the name of the first match. For existing SQLite files, detects by
    /// magic bytes even when no SQLite factory is in the registry.
    pub fn detect_backend(&self, locator: &str) -> Option<String> {
        if let Some(name) = self.registry.detect_backend(locator) {
            return Some(name.to_string());
        }
        #[cfg(feature = "sqlite")]
        {
            let path = std::path::Path::new(locator);
            if path.exists() {
                if let Ok(mut f) = std::fs::File::open(path) {
                    use std::io::Read;
                    let mut hdr = [0u8; 16];
                    let n = f.read(&mut hdr).unwrap_or(0);
                    if hdr[..n].starts_with(b"SQLite format 3\0") {
                        return Some("sqlite".to_string());
                    }
                }
            }
        }
        None
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

    /// Creates a [`KanbanBackend`] for `locator`, selecting SQLite or JSON
    /// automatically from the file content / extension.
    pub async fn make_backend(
        &self,
        locator: &str,
        config: &AppConfig,
    ) -> Result<std::sync::Arc<dyn crate::backend::KanbanBackend>, KanbanError> {
        if self.is_sqlite(locator) {
            #[cfg(feature = "sqlite")]
            {
                // Propagate via `?` (no stringification) so typed variants like
                // UnsupportedFutureVersion survive across make_backend to the
                // CLI / MCP / TUI surfaces, mirroring the JSON path's preserved
                // From<PersistenceError> for KanbanError mapping.
                let backend = kanban_persistence_sqlite::SqliteBackend::open(locator).await?;
                return Ok(std::sync::Arc::new(backend));
            }
            #[cfg(not(feature = "sqlite"))]
            return Err(KanbanError::Internal(format!(
                "path '{}' requires the sqlite feature which is not compiled in",
                locator
            )));
        }
        let store = self.make_store(config.effective_storage_backend(), locator)?;
        #[cfg(feature = "json")]
        return Ok(std::sync::Arc::new(
            kanban_persistence_json::JsonDataStore::new(store),
        ));
        #[cfg(not(feature = "json"))]
        Err(KanbanError::Internal(format!(
            "path '{}' requires the json feature which is not compiled in",
            locator
        )))
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

    /// Creates a store for `path`, verifies the file exists, then loads and
    /// deserializes the snapshot. Returns an error if the file is missing or
    /// the data cannot be parsed.
    ///
    /// For `.sqlite`/`.db` files, bypasses the registry and uses `SqliteStore`
    /// directly.
    pub async fn validate_and_load_store(
        &self,
        backend: &str,
        path: &str,
    ) -> Result<kanban_domain::Snapshot, KanbanError> {
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
                let store = kanban_persistence_sqlite::SqliteStore::open(path).await?;
                return store.snapshot();
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
        let data = snapshot_from_json_bytes(&snapshot.data)?;
        Ok(data)
    }

    /// Exports a board selection to a new SQLite file via `SqliteStore`.
    ///
    /// **Note:** The dependency graph is not part of the `AllBoardsExport` format
    /// and will not be present in the exported file.
    pub async fn export_to_sqlite(
        &self,
        export: kanban_domain::export::AllBoardsExport,
        filename: &str,
    ) -> Result<(), KanbanError> {
        #[cfg(feature = "sqlite")]
        {
            use kanban_domain::export::BoardImporter;
            use kanban_domain::{DependencyGraph, Snapshot};

            let entities = BoardImporter::extract_entities(export);
            let snapshot = Snapshot {
                archived_boards: entities.archived_boards,
                boards: entities.boards,
                columns: entities.columns,
                cards: entities.cards,
                archived_cards: entities.archived_cards,
                sprints: entities.sprints,
                graph: DependencyGraph::default(),
            };
            let store = kanban_persistence_sqlite::SqliteStore::open(filename).await?;
            store.apply_snapshot(snapshot)?;
            Ok(())
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
    /// SQLite source/destination are handled directly via `SqliteStore`;
    /// JSON and other registry-backed backends go through the `StoreRegistry`.
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

        // Load snapshot from source into a StoreSnapshot (JSON bytes) for FK repair.
        let mut store_snapshot: StoreSnapshot = match from_backend {
            "sqlite" | "sqlite3" | "db" => {
                #[cfg(feature = "sqlite")]
                {
                    use kanban_persistence::PersistenceMetadata;
                    // KAN-845: opening a SQLite source below SUPPORTED_SCHEMA_VERSION
                    // runs the same in-place schema upgrade (+ durable
                    // `.v{N}.backup` snapshot) that any other kanban binary
                    // would run against this file. That's intentional, not a
                    // migrate_store-specific side effect: reading a
                    // schema-current snapshot below requires the upgrade to
                    // have already run, and the source file gets exactly the
                    // same treatment `SqliteStore::open` gives it anywhere
                    // else it's opened directly.
                    let store = kanban_persistence_sqlite::SqliteStore::open(from_path).await?;
                    let snapshot = store.snapshot()?;
                    let data = kanban_persistence::snapshot_to_json_bytes(&snapshot)?;
                    StoreSnapshot {
                        data,
                        metadata: PersistenceMetadata::new(uuid::Uuid::new_v4()),
                    }
                }
                #[cfg(not(feature = "sqlite"))]
                return Err(KanbanError::validation("sqlite feature not compiled in"));
            }
            _ => {
                let source = self.make_store(from_backend, from_path)?;
                let (snap, _) = source.load().await?;
                snap
            }
        };

        repair_snapshot_fks(&mut store_snapshot)?;

        // Save to destination
        match to_backend {
            "sqlite" | "sqlite3" | "db" => {
                #[cfg(feature = "sqlite")]
                {
                    let repaired = snapshot_from_json_bytes(&store_snapshot.data)?;
                    let store = kanban_persistence_sqlite::SqliteStore::open(to_path).await?;
                    let outcome = store.apply_snapshot(repaired.clone());
                    store.close().await;
                    drop(store);
                    if let Err(e) = outcome {
                        cleanup_destination_files(to_path).await;
                        return Err(e);
                    }
                }
                #[cfg(not(feature = "sqlite"))]
                return Err(KanbanError::validation("sqlite feature not compiled in"));
            }
            _ => {
                let target = self.make_store(to_backend, to_path)?;
                let outcome = target.save(store_snapshot).await;
                target.close().await;
                drop(target);
                if let Err(e) = outcome {
                    cleanup_destination_files(to_path).await;
                    return Err(e.into());
                }
            }
        }
        Ok(())
    }
}

impl Clone for StoreManager {
    fn clone(&self) -> Self {
        Self {
            registry: Arc::clone(&self.registry),
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
async fn cleanup_destination_files(to_path: &str) {
    remove_file_with_windows_retry(std::path::Path::new(to_path)).await;
    let wal = format!("{}-wal", to_path);
    let shm = format!("{}-shm", to_path);
    remove_file_with_windows_retry(std::path::Path::new(&wal)).await;
    remove_file_with_windows_retry(std::path::Path::new(&shm)).await;
}

fn repair_snapshot_fks(snapshot: &mut StoreSnapshot) -> Result<(), KanbanError> {
    let mut data: serde_json::Value = serde_json::from_slice(&snapshot.data).map_err(|e| {
        KanbanError::validation(format!("Failed to parse snapshot for FK repair: {e}"))
    })?;

    let valid_columns: HashSet<String> = data["columns"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|c| c["id"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let valid_sprints: HashSet<String> = data["sprints"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s["id"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let fallback_column: Option<String> = data["columns"].as_array().and_then(|arr| {
        arr.iter()
            .min_by_key(|c| c["position"].as_i64().unwrap_or(i64::MAX))
            .and_then(|c| c["id"].as_str())
            .map(String::from)
    });

    if let Some(cards) = data["cards"].as_array_mut() {
        for card in cards.iter_mut() {
            fix_card_fks(
                card,
                &valid_columns,
                &valid_sprints,
                fallback_column.as_deref(),
            );
        }
        // Live cards must land in a real column. The SQLite `cards.column_id`
        // FK used to reject an orphan on save; it was dropped (KAN-832) so an
        // archived card can keep a dangling historical column, which also
        // removed that backstop for LIVE cards. `fix_card_fks` moved every
        // fixable card to the fallback column, so a card still pointing outside
        // `valid_columns` had no column to reassign to and is unrepairable —
        // fail explicitly (archived cards are intentionally exempt: their
        // column reference is historical and may dangle).
        let orphaned = cards
            .iter()
            .filter(|card| {
                card["column_id"]
                    .as_str()
                    .is_some_and(|c| !valid_columns.contains(c))
            })
            .count();
        if orphaned > 0 {
            return Err(KanbanError::validation(format!(
                "cannot migrate: {orphaned} live card(s) reference a column that does not \
                 exist and there is no column to reassign them to"
            )));
        }
    }

    // Archived cards are pure markers ({ entity_id, archived_at, board_id }); the
    // archived card's live row is repaired by the `cards` loop above. There is no
    // embedded `card` to fix here since the V10 migration ran before FK repair.

    snapshot.data = serde_json::to_vec(&data).map_err(|e| {
        KanbanError::validation(format!("Failed to serialize repaired snapshot: {e}"))
    })?;

    Ok(())
}

fn fix_card_fks(
    card: &mut serde_json::Value,
    valid_columns: &HashSet<String>,
    valid_sprints: &HashSet<String>,
    fallback_column: Option<&str>,
) {
    if let Some(sprint_id) = card["sprint_id"].as_str() {
        if !valid_sprints.contains(sprint_id) {
            card["sprint_id"] = serde_json::Value::Null;
        }
    }
    if let Some(col_id) = card["column_id"].as_str() {
        if !valid_columns.contains(col_id) {
            if let Some(fb) = fallback_column {
                card["column_id"] = serde_json::Value::String(fb.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_persistence::{PersistenceMetadata, StoreRegistry};
    use tempfile::tempdir;
    use uuid::Uuid;

    fn make_snapshot_with_json(data: serde_json::Value) -> StoreSnapshot {
        StoreSnapshot {
            data: serde_json::to_vec(&data).unwrap(),
            metadata: PersistenceMetadata::new(Uuid::new_v4()),
        }
    }

    /// Drift guard: verifies that an archived card whose live row has a dangling
    /// column_id gets the live row repaired (moved to fallback column) without
    /// erroring, and the marker entry is left unchanged with no `card` key.
    #[test]
    fn test_repair_snapshot_fks_repairs_live_row_of_archived_card() {
        let valid_col_id = Uuid::new_v4().to_string();
        let dangling_col_id = Uuid::new_v4().to_string();
        let card_id = Uuid::new_v4().to_string();
        let board_id = Uuid::new_v4().to_string();

        let data = serde_json::json!({
            "boards": [{"id": board_id}],
            "columns": [{"id": valid_col_id, "position": 0}],
            "sprints": [],
            "cards": [{
                "id": card_id,
                "column_id": dangling_col_id,
                "sprint_id": null
            }],
            "archived_cards": [{
                "entity_id": card_id,
                "board_id": board_id,
                "archived_at": "2024-01-01T00:00:00Z"
            }]
        });

        let mut snapshot = make_snapshot_with_json(data);
        repair_snapshot_fks(&mut snapshot)
            .expect("repair must succeed for archived card with dangling column");

        let repaired: serde_json::Value = serde_json::from_slice(&snapshot.data).unwrap();

        assert_eq!(
            repaired["cards"][0]["column_id"].as_str().unwrap(),
            valid_col_id,
            "live card row must be reassigned to fallback column"
        );
        let marker = &repaired["archived_cards"][0];
        assert!(
            marker.get("card").is_none(),
            "marker must have no embedded `card` key (pure marker shape)"
        );
        assert_eq!(marker["entity_id"].as_str().unwrap(), card_id);
        assert_eq!(marker["board_id"].as_str().unwrap(), board_id);
    }

    /// Drift guard: verifies that a marker-only archived_cards entry passes
    /// through repair_snapshot_fks byte-identical. Pins the marker-shape contract
    /// so a future reader cannot silently re-add an embed-handling branch.
    #[test]
    fn test_repair_snapshot_fks_marker_archived_cards_pass_through_unchanged() {
        let col_id = Uuid::new_v4().to_string();
        let card_id = Uuid::new_v4().to_string();
        let board_id = Uuid::new_v4().to_string();

        let archived_entry = serde_json::json!({
            "entity_id": card_id,
            "board_id": board_id,
            "archived_at": "2024-01-01T00:00:00Z"
        });

        let data = serde_json::json!({
            "boards": [],
            "columns": [{"id": col_id, "position": 0}],
            "sprints": [],
            "cards": [{"id": card_id, "column_id": col_id, "sprint_id": null}],
            "archived_cards": [archived_entry.clone()]
        });

        let mut snapshot = make_snapshot_with_json(data);
        repair_snapshot_fks(&mut snapshot).expect("repair must succeed");

        let repaired: serde_json::Value = serde_json::from_slice(&snapshot.data).unwrap();
        let marker_after = &repaired["archived_cards"][0];

        assert_eq!(
            marker_after["entity_id"].as_str().unwrap(),
            card_id,
            "entity_id must be unchanged"
        );
        assert_eq!(
            marker_after["board_id"].as_str().unwrap(),
            board_id,
            "board_id must be unchanged"
        );
        assert!(
            marker_after.get("card").is_none(),
            "no `card` embed must be present before or after repair"
        );
    }

    fn make_sm() -> StoreManager {
        let mut registry = StoreRegistry::new();
        #[cfg(feature = "sqlite")]
        registry.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
        #[cfg(feature = "json")]
        registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
        StoreManager::new(registry)
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
    }
}
