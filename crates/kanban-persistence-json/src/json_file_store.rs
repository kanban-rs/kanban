use crate::atomic_writer::AtomicWriter;
use crate::conflict::FileMetadata;
use crate::migration::{
    transform_to_v6_split_graph_value, transform_v10_to_v11_value, transform_v11_to_v12_value,
    transform_v12_to_v13_value, transform_v13_to_v14_value, transform_v14_to_v15_value,
    transform_v15_to_v16_value, transform_v16_to_v17_value, transform_v2_to_v3_value,
    transform_v6_to_v7_value, transform_v7_to_v8_value, transform_v8_to_v9_value,
    transform_v9_to_v10_value, Migrator,
};
use kanban_persistence::{
    FormatVersion, PersistenceError, PersistenceMetadata, PersistenceResult, PersistenceStore,
    StoreSnapshot,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

/// JSON file-based persistence store
/// Implements the PersistenceStore trait for JSON file operations
pub struct JsonFileStore {
    path: PathBuf,
    instance_id: Uuid,
    last_known_metadata: Mutex<Option<FileMetadata>>,
}

/// Wrapper structure for the JSON file format (v2+).
///
/// Pre-KAN-405 fields (`commands`, `undo_cursor`, `baseline_data`,
/// `command_schema_version`) are tolerated on deserialize so old files load
/// cleanly, then actively scrubbed from disk by `load`/`load_sync` — see
/// [`LEGACY_FIELDS`] and `scrub_legacy_fields`. Do NOT add
/// `#[serde(deny_unknown_fields)]` here: it would break the load path for
/// any file written by an older build.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonEnvelope {
    version: u32,
    metadata: PersistenceMetadata,
    data: serde_json::Value,
}

/// Top-level fields that pre-KAN-405 builds wrote alongside the envelope and
/// that this build actively removes when loading.
const LEGACY_FIELDS: &[&str] = &[
    "commands",
    "undo_cursor",
    "baseline_data",
    "command_schema_version",
];

fn detect_legacy_fields(value: &serde_json::Value) -> Vec<&'static str> {
    let Some(obj) = value.as_object() else {
        return Vec::new();
    };
    LEGACY_FIELDS
        .iter()
        .copied()
        .filter(|f| obj.contains_key(*f))
        .collect()
}

impl JsonEnvelope {
    /// Create a new V2 format envelope with the given data
    pub fn new(data: serde_json::Value) -> Self {
        Self {
            version: 2,
            metadata: PersistenceMetadata::new(Uuid::new_v4()),
            data,
        }
    }

    /// Create an empty V2 format envelope with default structure
    pub fn empty() -> Self {
        Self::new(serde_json::json!({
            "boards": [],
            "columns": [],
            "cards": [],
            "archived_cards": [],
            "sprints": []
        }))
    }

    /// Serialize to pretty-printed JSON string
    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

// ─── Sync migration helpers ───────────────────────────────────────────────────

/// Synchronous V*→latest migration chain used by [`JsonFileStore::load_sync`].
/// See [`migration::backup`] for the backup-path policy shared with the
/// async [`Migrator::migrate`] orchestrator.
fn migrate_to_latest_sync(from: FormatVersion, path: &Path) -> PersistenceResult<Vec<u8>> {
    // Take the outer backup BEFORE any per-step migration runs. The chain
    // (V1→V2, V2→V3, split_graph, v6_to_v7_rename, v7_to_v8) overwrites the
    // file in place at each step; without this outer backup a mid-chain
    // failure would leave the user with a partially-transformed file and no
    // rollback artifact. The backup is removed only on full V→latest success.
    let backup_path = crate::migration::pre_latest_backup_path_for(from, path);
    if let Some(backup) = &backup_path {
        std::fs::copy(path, backup)?;
        tracing::info!("Created pre-migration backup at {}", backup.display());
    }

    let result = (|| -> PersistenceResult<Vec<u8>> {
        if from == FormatVersion::V1 {
            migrate_v1_to_v2_sync(path)?;
        }
        if from <= FormatVersion::V2 {
            migrate_v2_to_v3_sync(path)?;
        }
        run_split_and_upgrade_chain_sync(from, path)
    })();

    match (result, backup_path) {
        (Ok(bytes), Some(backup)) => {
            if let Err(e) = std::fs::remove_file(&backup) {
                tracing::warn!(
                    "Migration successful but failed to remove backup at {}: {}",
                    backup.display(),
                    e
                );
            } else {
                tracing::info!("Migration to the current version verified, backup removed");
            }
            Ok(bytes)
        }
        (Ok(bytes), None) => Ok(bytes),
        (Err(e), Some(backup)) => {
            tracing::error!(
                "Migration to the current version failed: {}. Backup preserved at {}",
                e,
                backup.display()
            );
            Err(e)
        }
        (Err(e), None) => Err(e),
    }
}

/// Sync sibling of [`Migrator::run_split_and_upgrade_chain`]. Runs the V6
/// split-graph transform (only if the file is pre-V6), the v6→v7
/// spawns-bucket rename, the v7→v8 archived-cards backfill, the v8→v9
/// archived-boards bump, the v9→v10 archival reference-marker collapse, the
/// v10→v11 cards.board_id backfill, the v11→v12 completion_column_ids
/// backfill, the v12→v13 default_status backfill, then the v13→v14
/// default_status derivation, returning the final on-disk bytes.
fn run_split_and_upgrade_chain_sync(
    from: FormatVersion,
    path: &Path,
) -> PersistenceResult<Vec<u8>> {
    if from < FormatVersion::V6 {
        split_graph_sync(path)?;
    }
    v6_to_v7_rename_sync(path)?;
    v7_to_v8_archived_cards_sync(path)?;
    v8_to_v9_archived_boards_sync(path)?;
    v9_to_v10_archival_refs_sync(path)?;
    v10_to_v11_card_board_id_sync(path)?;
    v11_to_v12_completion_columns_sync(path)?;
    v12_to_v13_column_default_status_sync(path)?;
    v13_to_v14_default_status_derivation_sync(path)?;
    v14_to_v15_prefixes_sync(path)?;
    v15_to_v16_card_prefix_sync(path)?;
    v16_to_v17_drop_legacy_counters_sync(path)
}

fn migrate_v1_to_v2_sync(path: &Path) -> PersistenceResult<Vec<u8>> {
    // Per-step backup removed: the outer migrate_to_latest_sync wrap owns the
    // .v1.backup now and keeps it for the entire chain, not just this step.
    let content = std::fs::read_to_string(path)?;
    let v1_data: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    let v2_envelope = JsonEnvelope::new(v1_data);
    let json_str = v2_envelope
        .to_json_string()
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    let json_bytes = json_str.into_bytes();
    AtomicWriter::write_atomic_sync(path, &json_bytes)?;
    tracing::info!("Migrated {} from V1 to V2 (sync)", path.display());
    Ok(json_bytes)
}

fn migrate_v2_to_v3_sync(path: &Path) -> PersistenceResult<Vec<u8>> {
    let content = std::fs::read_to_string(path)?;
    let mut envelope: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    transform_v2_to_v3_value(&mut envelope)?;
    let json_str = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    let json_bytes = json_str.into_bytes();
    AtomicWriter::write_atomic_sync(path, &json_bytes)?;
    tracing::info!("Migrated {} from V2 to V3 (sync)", path.display());
    Ok(json_bytes)
}

fn v8_to_v9_archived_boards_sync(path: &Path) -> PersistenceResult<Vec<u8>> {
    let content = std::fs::read_to_string(path)?;
    let mut envelope: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v8_to_v9_value(&mut envelope)? {
        return Ok(content.into_bytes());
    }
    let json_str = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    let json_bytes = json_str.into_bytes();
    AtomicWriter::write_atomic_sync(path, &json_bytes)?;
    tracing::info!("Migrated {} from V8 to V9 (sync)", path.display());
    Ok(json_bytes)
}

fn v9_to_v10_archival_refs_sync(path: &Path) -> PersistenceResult<Vec<u8>> {
    let content = std::fs::read_to_string(path)?;
    let mut envelope: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v9_to_v10_value(&mut envelope)? {
        return Ok(content.into_bytes());
    }
    let json_str = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    let json_bytes = json_str.into_bytes();
    AtomicWriter::write_atomic_sync(path, &json_bytes)?;
    tracing::info!(
        "Migrated {} from V9 to V10 (archival reference-marker collapse, sync)",
        path.display()
    );
    Ok(json_bytes)
}

fn v10_to_v11_card_board_id_sync(path: &Path) -> PersistenceResult<Vec<u8>> {
    let content = std::fs::read_to_string(path)?;
    let mut envelope: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v10_to_v11_value(&mut envelope)? {
        return Ok(content.into_bytes());
    }
    let json_str = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    let json_bytes = json_str.into_bytes();
    AtomicWriter::write_atomic_sync(path, &json_bytes)?;
    tracing::info!(
        "Migrated {} from V10 to V11 (cards.board_id backfill, sync)",
        path.display()
    );
    Ok(json_bytes)
}

fn v11_to_v12_completion_columns_sync(path: &Path) -> PersistenceResult<Vec<u8>> {
    let content = std::fs::read_to_string(path)?;
    let mut envelope: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v11_to_v12_value(&mut envelope)? {
        return Ok(content.into_bytes());
    }
    let json_str = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    let json_bytes = json_str.into_bytes();
    AtomicWriter::write_atomic_sync(path, &json_bytes)?;
    tracing::info!(
        "Migrated {} from V11 to V12 (completion_column_ids backfill, sync)",
        path.display()
    );
    Ok(json_bytes)
}

fn v12_to_v13_column_default_status_sync(path: &Path) -> PersistenceResult<Vec<u8>> {
    let content = std::fs::read_to_string(path)?;
    let mut envelope: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v12_to_v13_value(&mut envelope)? {
        return Ok(content.into_bytes());
    }
    let json_str = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    let json_bytes = json_str.into_bytes();
    AtomicWriter::write_atomic_sync(path, &json_bytes)?;
    tracing::info!(
        "Migrated {} from V12 to V13 (default_status backfill, sync)",
        path.display()
    );
    Ok(json_bytes)
}

fn v13_to_v14_default_status_derivation_sync(path: &Path) -> PersistenceResult<Vec<u8>> {
    let content = std::fs::read_to_string(path)?;
    let mut envelope: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v13_to_v14_value(&mut envelope)? {
        return Ok(content.into_bytes());
    }
    let json_str = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    let json_bytes = json_str.into_bytes();
    AtomicWriter::write_atomic_sync(path, &json_bytes)?;
    tracing::info!(
        "Migrated {} from V13 to V14 (default_status derivation, sync)",
        path.display()
    );
    Ok(json_bytes)
}

fn v14_to_v15_prefixes_sync(path: &Path) -> PersistenceResult<Vec<u8>> {
    let content = std::fs::read_to_string(path)?;
    let mut envelope: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v14_to_v15_value(&mut envelope)? {
        return Ok(content.into_bytes());
    }
    let json_str = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    let json_bytes = json_str.into_bytes();
    AtomicWriter::write_atomic_sync(path, &json_bytes)?;
    tracing::info!(
        "Migrated {} from V14 to V15 (prefixes backfill, sync)",
        path.display()
    );
    Ok(json_bytes)
}

fn v16_to_v17_drop_legacy_counters_sync(path: &Path) -> PersistenceResult<Vec<u8>> {
    let content = std::fs::read_to_string(path)?;
    let mut envelope: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v16_to_v17_value(&mut envelope)? {
        return Ok(content.into_bytes());
    }
    let json_str = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    let json_bytes = json_str.into_bytes();
    AtomicWriter::write_atomic_sync(path, &json_bytes)?;
    tracing::info!(
        "Migrated {} from V16 to V17 (legacy counters dropped, sync)",
        path.display()
    );
    Ok(json_bytes)
}

fn v15_to_v16_card_prefix_sync(path: &Path) -> PersistenceResult<Vec<u8>> {
    let content = std::fs::read_to_string(path)?;
    let mut envelope: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v15_to_v16_value(&mut envelope)? {
        return Ok(content.into_bytes());
    }
    let json_str = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    let json_bytes = json_str.into_bytes();
    AtomicWriter::write_atomic_sync(path, &json_bytes)?;
    tracing::info!(
        "Migrated {} from V15 to V16 (card prefix backfill, sync)",
        path.display()
    );
    Ok(json_bytes)
}

fn split_graph_sync(path: &Path) -> PersistenceResult<Vec<u8>> {
    let content = std::fs::read_to_string(path)?;
    let mut envelope: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    transform_to_v6_split_graph_value(&mut envelope)?;
    let json_str = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    let json_bytes = json_str.into_bytes();
    AtomicWriter::write_atomic_sync(path, &json_bytes)?;
    tracing::info!("Applied split-graph migration to {} (sync)", path.display());
    Ok(json_bytes)
}

fn v6_to_v7_rename_sync(path: &Path) -> PersistenceResult<Vec<u8>> {
    let content = std::fs::read_to_string(path)?;
    let mut envelope: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    transform_v6_to_v7_value(&mut envelope)?;
    let json_str = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    let json_bytes = json_str.into_bytes();
    AtomicWriter::write_atomic_sync(path, &json_bytes)?;
    tracing::info!(
        "Applied v6→v7 spawns-rename migration to {} (sync)",
        path.display()
    );
    Ok(json_bytes)
}

fn v7_to_v8_archived_cards_sync(path: &Path) -> PersistenceResult<Vec<u8>> {
    let content = std::fs::read_to_string(path)?;
    let mut envelope: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    transform_v7_to_v8_value(&mut envelope)?;
    let json_str = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    let json_bytes = json_str.into_bytes();
    AtomicWriter::write_atomic_sync(path, &json_bytes)?;
    tracing::info!(
        "Applied v7→v8 archived-cards board_id backfill to {} (sync)",
        path.display()
    );
    Ok(json_bytes)
}

// ─────────────────────────────────────────────────────────────────────────────

impl JsonFileStore {
    /// Create a new JSON file store
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            instance_id: Uuid::new_v4(),
            last_known_metadata: Mutex::new(None),
        }
    }

    /// Create a new JSON file store with a specific instance ID
    /// (useful for testing or coordinating across instances)
    pub fn with_instance_id(path: impl AsRef<Path>, instance_id: Uuid) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            instance_id,
            last_known_metadata: Mutex::new(None),
        }
    }

    /// Get the instance ID for this store
    pub fn instance_id(&self) -> Uuid {
        self.instance_id
    }

    fn lock_metadata(&self) -> PersistenceResult<std::sync::MutexGuard<'_, Option<FileMetadata>>> {
        self.last_known_metadata
            .lock()
            .map_err(|e| PersistenceError::Serialization(format!("Metadata mutex poisoned: {e}")))
    }

    /// Parse file bytes into a [`JsonEnvelope`]. Version validation is the
    /// caller's responsibility — `Migrator::detect_version_from_value` is
    /// called before this in both load paths and refuses future / malformed
    /// versions. No defence-in-depth duplication here.
    fn parse_envelope(bytes: &[u8]) -> PersistenceResult<JsonEnvelope> {
        serde_json::from_slice(bytes).map_err(|e| PersistenceError::Serialization(e.to_string()))
    }

    fn serialize_envelope(envelope: &JsonEnvelope) -> PersistenceResult<Vec<u8>> {
        serde_json::to_vec_pretty(envelope)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))
    }

    async fn scrub_legacy_fields_async(
        &self,
        envelope: &JsonEnvelope,
        detected: &[&'static str],
    ) -> PersistenceResult<()> {
        tracing::info!(
            "scrubbing pre-KAN-405 legacy fields {:?} from {}; undo history is now in-session only",
            detected,
            self.path.display()
        );
        let bytes = Self::serialize_envelope(envelope)?;
        AtomicWriter::write_atomic(&self.path, &bytes).await?;
        Ok(())
    }

    fn scrub_legacy_fields_sync(
        &self,
        envelope: &JsonEnvelope,
        detected: &[&'static str],
    ) -> PersistenceResult<()> {
        tracing::info!(
            "scrubbing pre-KAN-405 legacy fields {:?} from {} (sync); undo history is now in-session only",
            detected,
            self.path.display()
        );
        let bytes = Self::serialize_envelope(envelope)?;
        AtomicWriter::write_atomic_sync(&self.path, &bytes)?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl PersistenceStore for JsonFileStore {
    async fn save(&self, mut snapshot: StoreSnapshot) -> PersistenceResult<PersistenceMetadata> {
        // Check for external file modifications before saving
        if self.path.exists() {
            let current_metadata =
                FileMetadata::from_file(&self.path).map_err(PersistenceError::Io)?;

            // Compare with last known metadata
            let guard = self.lock_metadata()?;
            if let Some(last_known) = *guard {
                if last_known != current_metadata {
                    return Err(PersistenceError::ConflictDetected {
                        path: self.path.to_string_lossy().to_string(),
                        source: None,
                    });
                }
            }
        }

        // Update metadata with current instance, time, and writer identity
        snapshot.metadata.instance_id = self.instance_id;
        snapshot.metadata.saved_at = chrono::Utc::now();
        snapshot.metadata.writer_version = Some(kanban_core::KANBAN_VERSION.to_string());
        snapshot.metadata.writer_commit = Some(kanban_core::KANBAN_COMMIT.to_string());

        let data_value: serde_json::Value = serde_json::from_slice(&snapshot.data)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        let envelope = JsonEnvelope {
            version: FormatVersion::MAX.as_u32(),
            metadata: snapshot.metadata.clone(),
            data: data_value,
        };

        // Serialize envelope to JSON
        let json_bytes = serde_json::to_vec_pretty(&envelope)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        // Write atomically to disk for crash safety
        AtomicWriter::write_atomic(&self.path, &json_bytes).await?;

        // Update last known metadata after successful write
        if let Ok(new_metadata) = FileMetadata::from_file(&self.path) {
            let mut guard = self.lock_metadata()?;
            *guard = Some(new_metadata);
        }

        tracing::info!(
            "Saved {} bytes to {}",
            json_bytes.len(),
            self.path.display()
        );

        Ok(snapshot.metadata)
    }

    async fn load(&self) -> PersistenceResult<(StoreSnapshot, PersistenceMetadata)> {
        let current_version = Migrator::detect_version(&self.path).await?;

        if current_version < FormatVersion::MAX {
            tracing::info!(
                "Detected {:?} format at {}. Migrating to current...",
                current_version,
                self.path.display()
            );
            Migrator::migrate(current_version, FormatVersion::MAX, &self.path).await?;
            tracing::info!("Migration to current format completed successfully");
        }

        let file_bytes = tokio::fs::read(&self.path).await?;
        let envelope = Self::parse_envelope(&file_bytes)?;

        let raw_value: serde_json::Value = serde_json::from_slice(&file_bytes)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        let detected = detect_legacy_fields(&raw_value);
        if !detected.is_empty() {
            if let Err(e) = self.scrub_legacy_fields_async(&envelope, &detected).await {
                tracing::warn!(
                    "failed to scrub legacy fields from {}: {}; data still loaded successfully, cleanup will be retried on next open",
                    self.path.display(),
                    e
                );
            }
        }

        // The migration chain above upgrades any pre-current file to the
        // current format (V10 lifts embedded archived entities to pure
        // reference markers on disk, V11 backfills cards.board_id, V12
        // backfills completion_column_ids); a current-format file already
        // deserializes directly into the collapsed `Snapshot`.
        let data = serde_json::to_vec(&envelope.data)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        let mut metadata = envelope.metadata;
        metadata.format_version = Some(envelope.version);
        let snapshot = StoreSnapshot {
            data,
            metadata: metadata.clone(),
        };

        if let Ok(file_metadata) = FileMetadata::from_file(&self.path) {
            let mut guard = self.lock_metadata()?;
            *guard = Some(file_metadata);
        }

        tracing::info!(
            "Loaded {} bytes from {}",
            file_bytes.len(),
            self.path.display()
        );

        Ok((snapshot, metadata))
    }

    fn load_sync(&self) -> PersistenceResult<Option<(StoreSnapshot, PersistenceMetadata)>> {
        if !self.path.exists() {
            return Ok(None);
        }

        let file_bytes = std::fs::read(&self.path)?;
        let value: serde_json::Value = serde_json::from_slice(&file_bytes)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let current_version = Migrator::detect_version_from_value(&value)?;

        let final_bytes = if current_version < FormatVersion::MAX {
            tracing::info!(
                "Detected {:?} format at {}. Migrating to current (sync)...",
                current_version,
                self.path.display()
            );
            migrate_to_latest_sync(current_version, &self.path)?
        } else {
            file_bytes
        };

        let envelope = Self::parse_envelope(&final_bytes)?;

        let raw_value: serde_json::Value = serde_json::from_slice(&final_bytes)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        let detected = detect_legacy_fields(&raw_value);
        if !detected.is_empty() {
            if let Err(e) = self.scrub_legacy_fields_sync(&envelope, &detected) {
                tracing::warn!(
                    "failed to scrub legacy fields from {} (sync): {}; data still loaded successfully, cleanup will be retried on next open",
                    self.path.display(),
                    e
                );
            }
        }

        // The migration chain above upgrades any pre-current file to the
        // current format (lifting embedded archived entities to reference
        // markers on disk, backfilling cards.board_id); a current-format file
        // deserializes directly into the collapsed `Snapshot`.
        let data = serde_json::to_vec(&envelope.data)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        let mut metadata = envelope.metadata;
        metadata.format_version = Some(envelope.version);
        let snapshot = StoreSnapshot {
            data,
            metadata: metadata.clone(),
        };

        if let Ok(file_metadata) = FileMetadata::from_file(&self.path) {
            let mut guard = self.lock_metadata()?;
            *guard = Some(file_metadata);
        }

        tracing::info!(
            "Loaded {} bytes from {} (sync)",
            final_bytes.len(),
            self.path.display()
        );

        Ok(Some((snapshot, metadata)))
    }

    async fn exists(&self) -> bool {
        self.path.exists()
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn instance_id(&self) -> Uuid {
        self.instance_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_save_and_load() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.json");
        let store = JsonFileStore::new(&file_path);

        let data = json!({ "boards": [], "columns": [] });
        let snapshot = StoreSnapshot {
            data: serde_json::to_vec(&data).unwrap(),
            metadata: PersistenceMetadata::new(store.instance_id()),
        };

        // Save
        let _metadata = store.save(snapshot.clone()).await.unwrap();
        assert!(file_path.exists());

        // Load
        let (loaded_snapshot, _loaded_metadata) = store.load().await.unwrap();

        let loaded_data: serde_json::Value = serde_json::from_slice(&loaded_snapshot.data).unwrap();
        assert_eq!(loaded_data, data);
    }

    #[tokio::test]
    async fn test_exists() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("nonexistent.json");
        let store = JsonFileStore::new(&file_path);

        assert!(!store.exists().await);

        // Create file
        let data = json!({});
        let snapshot = StoreSnapshot {
            data: serde_json::to_vec(&data).unwrap(),
            metadata: PersistenceMetadata::new(store.instance_id()),
        };
        store.save(snapshot).await.unwrap();

        assert!(store.exists().await);
    }

    #[test]
    fn test_json_envelope_empty_structure() {
        let envelope = JsonEnvelope::empty();
        let json = serde_json::to_value(envelope).unwrap();

        assert_eq!(json["version"], 2);
        assert!(json["metadata"].is_object());
        assert!(json["data"]["boards"].is_array());
        assert!(json["data"]["columns"].is_array());
        assert!(json["data"]["cards"].is_array());
        assert!(json["data"]["archived_cards"].is_array());
        assert!(json["data"]["sprints"].is_array());
    }

    #[test]
    fn test_lock_metadata_returns_result_not_panic() {
        let store = JsonFileStore::new("/tmp/nonexistent.json");
        let guard = store.lock_metadata();
        assert!(guard.is_ok());
        assert!(guard.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_load_rejects_future_format_version() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("future.json");
        let v99 = json!({
            "version": 99,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2026-05-23T00:00:00Z"
            },
            "data": {}
        });
        tokio::fs::write(&file_path, v99.to_string()).await.unwrap();

        let store = JsonFileStore::new(&file_path);
        let err = store.load().await.expect_err("v99 must refuse to load");
        assert!(
            matches!(
                err,
                PersistenceError::UnsupportedFutureVersion {
                    file_version: 99,
                    binary_max: 17
                }
            ),
            "expected UnsupportedFutureVersion, got: {err:?}"
        );
    }

    #[test]
    fn test_load_sync_rejects_future_format_version() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("future.json");
        let v99 = json!({
            "version": 99,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2026-05-23T00:00:00Z"
            },
            "data": {}
        });
        std::fs::write(&file_path, v99.to_string()).unwrap();

        let store = JsonFileStore::new(&file_path);
        let err = store
            .load_sync()
            .expect_err("v99 must refuse to load (sync)");
        assert!(
            matches!(
                err,
                PersistenceError::UnsupportedFutureVersion {
                    file_version: 99,
                    binary_max: 17
                }
            ),
            "expected UnsupportedFutureVersion, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn test_save_stamps_writer_version_and_commit() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("stamped.json");
        let store = JsonFileStore::new(&file_path);

        let snapshot = StoreSnapshot {
            data: serde_json::to_vec(&json!({ "boards": [], "columns": [] })).unwrap(),
            metadata: PersistenceMetadata::new(store.instance_id()),
        };
        store.save(snapshot).await.unwrap();

        let bytes = tokio::fs::read(&file_path).await.unwrap();
        let envelope: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            envelope["metadata"]["writer_version"]
                .as_str()
                .map(str::to_string),
            Some(kanban_core::KANBAN_VERSION.to_string()),
        );
        assert_eq!(
            envelope["metadata"]["writer_commit"]
                .as_str()
                .map(str::to_string),
            Some(kanban_core::KANBAN_COMMIT.to_string()),
        );
    }

    #[tokio::test]
    async fn test_load_legacy_file_without_writer_stamp_succeeds() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("legacy_no_stamp.json");
        let legacy = json!({
            "version": 6,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [], "columns": [], "cards": [], "archived_cards": [], "sprints": [],
                "graph": {
                    "parent_child": { "edges": [] },
                    "blocks": { "edges": [] },
                    "relates": { "edges": [] }
                }
            }
        });
        tokio::fs::write(&file_path, legacy.to_string())
            .await
            .unwrap();

        let store = JsonFileStore::new(&file_path);
        let (_, metadata) = store.load().await.unwrap();
        assert!(metadata.writer_version.is_none());
        assert!(metadata.writer_commit.is_none());
    }

    /// V6 files on disk used `parent_child` as the spawns-graph bucket key.
    /// V7 renames the bucket to `spawns` so the wire format matches the
    /// rest of the codebase (`SpawnsEdge`, `spawns_edges()`, SQLite
    /// `spawns_edges` table). Loading a V6 file must migrate it to V7
    /// on disk and surface the edges correctly through the deserialised
    /// `DependencyGraph` (whose field is now `spawns`, not `parent_child`).
    #[tokio::test]
    async fn test_load_v6_file_with_parent_child_migrates_to_v7_spawns() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("v6_parent_child.json");
        let parent = "550e8400-e29b-41d4-a716-446655440011";
        let child = "550e8400-e29b-41d4-a716-446655440012";
        let v6 = json!({
            "version": 6,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [], "columns": [], "cards": [], "archived_cards": [], "sprints": [],
                "graph": {
                    "parent_child": { "edges": [{
                        "source": parent,
                        "target": child,
                        "created_at": "2024-01-01T00:00:00Z",
                        "archived_at": null
                    }]},
                    "blocks": { "edges": [] },
                    "relates": { "edges": [] }
                }
            }
        });
        tokio::fs::write(&file_path, v6.to_string()).await.unwrap();

        let store = JsonFileStore::new(&file_path);
        let _ = store.load().await.unwrap();

        let after = tokio::fs::read_to_string(&file_path).await.unwrap();
        let v: serde_json::Value = serde_json::from_str(&after).unwrap();
        assert_eq!(v["version"], 17, "load must migrate V6 to current on disk");
        let graph = v["data"]["graph"].as_object().expect("graph object");
        assert!(
            graph.contains_key("spawns"),
            "V7 graph bucket key must be `spawns`; got {:?}",
            graph.keys().collect::<Vec<_>>()
        );
        assert!(
            !graph.contains_key("parent_child"),
            "legacy `parent_child` key must be gone after V7 migration"
        );
        let edges = graph["spawns"]["edges"]
            .as_array()
            .expect("spawns edges array");
        assert_eq!(
            edges.len(),
            1,
            "the original parent_child edge must survive"
        );
        assert_eq!(edges[0]["source"], parent);
        assert_eq!(edges[0]["target"], child);
    }

    /// Sync analogue of `test_load_v6_file_with_parent_child_migrates_to_v7_spawns`.
    /// Both entry points (`load` and `load_sync`) must apply the V6 -> V7
    /// rename with the same observable result, otherwise non-async callers
    /// silently keep loading the legacy bucket.
    #[test]
    fn test_load_sync_v6_file_with_parent_child_migrates_to_v7_spawns() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("v6_sync.json");
        let parent = "550e8400-e29b-41d4-a716-446655440021";
        let child = "550e8400-e29b-41d4-a716-446655440022";
        let v6 = json!({
            "version": 6,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440020",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [], "columns": [], "cards": [], "archived_cards": [], "sprints": [],
                "graph": {
                    "parent_child": { "edges": [{
                        "source": parent,
                        "target": child,
                        "created_at": "2024-01-01T00:00:00Z",
                        "archived_at": null
                    }]},
                    "blocks": { "edges": [] },
                    "relates": { "edges": [] }
                }
            }
        });
        std::fs::write(&file_path, v6.to_string()).unwrap();

        let store = JsonFileStore::new(&file_path);
        let _ = store.load_sync().unwrap().expect("file exists");

        let after = std::fs::read_to_string(&file_path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&after).unwrap();
        assert_eq!(
            v["version"], 17,
            "load_sync must migrate V6 to current on disk"
        );
        let graph = v["data"]["graph"].as_object().expect("graph object");
        assert!(
            graph.contains_key("spawns"),
            "V7 graph bucket key must be `spawns`; got {:?}",
            graph.keys().collect::<Vec<_>>()
        );
        assert!(
            !graph.contains_key("parent_child"),
            "legacy `parent_child` key must be gone after V7 migration"
        );
        let edges = graph["spawns"]["edges"]
            .as_array()
            .expect("spawns edges array");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["source"], parent);
        assert_eq!(edges[0]["target"], child);
    }

    fn v7_fixture_with_archived_card(
        board: &str,
        column: &str,
        original_column: &str,
    ) -> serde_json::Value {
        json!({
            "version": 7,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [{ "id": board, "name": "B" }],
                "columns": [{ "id": column, "board_id": board }],
                "cards": [],
                "archived_cards": [{
                    "card": { "id": "33333333-3333-3333-3333-333333333333", "title": "T" },
                    "archived_at": "2024-01-01T00:00:00Z",
                    "original_column_id": original_column,
                    "original_position": 0
                }],
                "sprints": [],
                "graph": {
                    "spawns": { "edges": [] },
                    "blocks": { "edges": [] },
                    "relates": { "edges": [] }
                }
            }
        })
    }

    /// D7 round-trip: a historical V7 file whose archived card has NO
    /// `board_id` must, on load, migrate to V9 on disk AND backfill the
    /// board_id from `original_column_id` -> column.board_id. Then a
    /// subsequent save+reload must preserve the backfilled value (byte-stable
    /// archived_cards), proving the migration is data-preserving.
    #[tokio::test]
    async fn test_json_archived_card_v7_file_migrates_and_backfills_board_id() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("v7_archived.json");
        let board = "11111111-1111-1111-1111-111111111111";
        let column = "22222222-2222-2222-2222-222222222222";
        let fixture = v7_fixture_with_archived_card(board, column, column);
        tokio::fs::write(&file_path, serde_json::to_string_pretty(&fixture).unwrap())
            .await
            .unwrap();

        let store = JsonFileStore::new(&file_path);
        let (snapshot, metadata) = store.load().await.unwrap();

        // On-disk: migrated to V8 with board_id backfilled.
        let on_disk: serde_json::Value =
            serde_json::from_slice(&tokio::fs::read(&file_path).await.unwrap()).unwrap();
        assert_eq!(
            on_disk["version"], 17,
            "load must migrate V7 to current on disk"
        );
        assert_eq!(
            on_disk["data"]["archived_cards"][0]["board_id"]
                .as_str()
                .unwrap(),
            board,
            "board_id must be backfilled from original_column_id"
        );

        // The loaded snapshot data carries the backfilled board_id.
        let loaded_data: serde_json::Value = serde_json::from_slice(&snapshot.data).unwrap();
        let archived_before: serde_json::Value = loaded_data["archived_cards"].clone();
        assert_eq!(archived_before[0]["board_id"].as_str().unwrap(), board);

        // Save the reloaded snapshot and read it back: archived_cards stable.
        store
            .save(StoreSnapshot {
                data: snapshot.data.clone(),
                metadata,
            })
            .await
            .unwrap();
        let (reloaded, _) = store.load().await.unwrap();
        let reloaded_data: serde_json::Value = serde_json::from_slice(&reloaded.data).unwrap();
        assert_eq!(
            reloaded_data["archived_cards"], archived_before,
            "archived_cards must survive save->reload byte-stable"
        );
    }

    /// A V7 archived card whose `original_column_id` no longer resolves to a
    /// column keeps a nil `board_id` (unrecoverable, acceptable) and the load
    /// still succeeds.
    #[tokio::test]
    async fn test_json_archived_card_v7_dangling_column_keeps_nil_board_id() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("v7_dangling.json");
        let board = "11111111-1111-1111-1111-111111111111";
        let column = "22222222-2222-2222-2222-222222222222";
        let missing = "99999999-9999-9999-9999-999999999999";
        let fixture = v7_fixture_with_archived_card(board, column, missing);
        tokio::fs::write(&file_path, serde_json::to_string_pretty(&fixture).unwrap())
            .await
            .unwrap();

        let store = JsonFileStore::new(&file_path);
        let (snapshot, _) = store.load().await.unwrap();

        let loaded_data: serde_json::Value = serde_json::from_slice(&snapshot.data).unwrap();
        assert_eq!(
            loaded_data["archived_cards"][0]["board_id"]
                .as_str()
                .unwrap(),
            uuid::Uuid::nil().to_string(),
            "a dangling original_column_id yields nil board_id, not a load failure"
        );
    }

    /// The V7->V8 migration writes a `.v7.backup` before the destructive
    /// step and removes it on success.
    #[tokio::test]
    async fn test_load_v7_to_v8_cleans_up_v7_backup_on_success() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("v7_backup.json");
        let board = "11111111-1111-1111-1111-111111111111";
        let column = "22222222-2222-2222-2222-222222222222";
        let fixture = v7_fixture_with_archived_card(board, column, column);
        tokio::fs::write(&file_path, serde_json::to_string_pretty(&fixture).unwrap())
            .await
            .unwrap();

        let store = JsonFileStore::new(&file_path);
        let _ = store.load().await.unwrap();

        assert!(
            !file_path.with_extension("v7.backup").exists(),
            ".v7.backup must be removed after a successful V7->V8 load"
        );
    }

    /// Sync analogue: `load_sync` must migrate V7->V8 and backfill board_id
    /// with the same observable result as the async path.
    #[test]
    fn test_load_sync_v7_to_v8_backfills_board_id() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("v7_sync.json");
        let board = "11111111-1111-1111-1111-111111111111";
        let column = "22222222-2222-2222-2222-222222222222";
        let fixture = v7_fixture_with_archived_card(board, column, column);
        std::fs::write(&file_path, serde_json::to_string_pretty(&fixture).unwrap()).unwrap();

        let store = JsonFileStore::new(&file_path);
        let (snapshot, _) = store.load_sync().unwrap().expect("file exists");

        let on_disk: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&file_path).unwrap()).unwrap();
        assert_eq!(
            on_disk["version"], 17,
            "load_sync must migrate V7 to current"
        );
        let loaded_data: serde_json::Value = serde_json::from_slice(&snapshot.data).unwrap();
        assert_eq!(
            loaded_data["archived_cards"][0]["board_id"]
                .as_str()
                .unwrap(),
            board
        );
        assert!(
            !file_path.with_extension("v7.backup").exists(),
            ".v7.backup must be removed after a successful V7->V8 load_sync"
        );
    }

    /// Files with stale `commands`/`undo_cursor`/`baseline_data`/
    /// `command_schema_version` fields (written by pre-KAN-405 builds) must
    /// be actively scrubbed from disk on load — not just ignored in memory.
    /// Serde would silently drop them on the next save, but that "lazy" cleanup
    /// leaves dust on disk until the user happens to mutate. The load path
    /// rewrites the file with a clean envelope as soon as legacy fields are
    /// detected so the cleanup is observable and guaranteed.
    #[tokio::test]
    async fn test_legacy_command_fields_are_scrubbed_from_disk_on_load() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("legacy.json");

        let legacy = json!({
            "version": 5,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [{"id": "550e8400-e29b-41d4-a716-446655440001", "name": "B",
                    "task_sort_field": "Default", "task_sort_order": "Ascending",
                    "sprint_name_used_count": 0, "next_sprint_number": 1,
                    "task_list_view": "Flat", "position": 0,
                    "created_at": "2024-01-01T00:00:00Z", "updated_at": "2024-01-01T00:00:00Z"}],
                "columns": [], "cards": [], "archived_cards": [], "sprints": [],
                "graph": { "cards": { "edges": [] } }
            },
            "commands": [{"type": "Board", "variant": "Create", "id": "00000000-0000-0000-0000-000000000001"}],
            "undo_cursor": 1,
            "command_schema_version": 1,
            "baseline_data": {}
        });
        tokio::fs::write(&file_path, legacy.to_string())
            .await
            .unwrap();

        let store = JsonFileStore::new(&file_path);
        let (snapshot, _meta) = store.load().await.unwrap();

        let loaded: serde_json::Value = serde_json::from_slice(&snapshot.data).unwrap();
        assert_eq!(loaded["boards"][0]["name"], "B", "board data must survive");

        let on_disk_bytes = tokio::fs::read(&file_path).await.unwrap();
        let on_disk: serde_json::Value = serde_json::from_slice(&on_disk_bytes).unwrap();
        let keys: Vec<_> = on_disk.as_object().unwrap().keys().cloned().collect();
        assert!(
            !keys.iter().any(|k| k == "commands"),
            "commands field must be scrubbed from disk, found keys: {keys:?}"
        );
        assert!(
            !keys.iter().any(|k| k == "undo_cursor"),
            "undo_cursor field must be scrubbed from disk, found keys: {keys:?}"
        );
        assert!(
            !keys.iter().any(|k| k == "baseline_data"),
            "baseline_data field must be scrubbed from disk, found keys: {keys:?}"
        );
        assert!(
            !keys.iter().any(|k| k == "command_schema_version"),
            "command_schema_version field must be scrubbed from disk, found keys: {keys:?}"
        );
        assert_eq!(
            on_disk["data"]["boards"][0]["name"], "B",
            "board data must remain on disk after scrub"
        );
    }

    /// `load_sync` must scrub legacy fields with the same guarantee as the
    /// async `load` — both are valid entry points and both must leave a clean
    /// file on disk.
    #[test]
    fn test_legacy_command_fields_are_scrubbed_from_disk_on_load_sync() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("legacy_sync.json");

        let legacy = json!({
            "version": 5,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [],
                "columns": [], "cards": [], "archived_cards": [], "sprints": [],
                "graph": { "cards": { "edges": [] } }
            },
            "commands": [],
            "undo_cursor": 0,
            "command_schema_version": 1,
            "baseline_data": {}
        });
        std::fs::write(&file_path, legacy.to_string()).unwrap();

        let store = JsonFileStore::new(&file_path);
        let _ = store.load_sync().unwrap().expect("file exists");

        let on_disk_bytes = std::fs::read(&file_path).unwrap();
        let on_disk: serde_json::Value = serde_json::from_slice(&on_disk_bytes).unwrap();
        let keys: Vec<_> = on_disk.as_object().unwrap().keys().cloned().collect();
        for legacy_key in [
            "commands",
            "undo_cursor",
            "baseline_data",
            "command_schema_version",
        ] {
            assert!(
                !keys.iter().any(|k| k == legacy_key),
                "{legacy_key} must be scrubbed from disk by load_sync, found keys: {keys:?}"
            );
        }
    }

    /// Loading a clean V7 file (current format) that has no legacy fields
    /// must not rewrite it. A spurious write would change the file's
    /// mtime, trip file-watcher notifications, and risk altering
    /// byte-for-byte content (which some users may track in version
    /// control). Pre-V7 files are migrated on load and *are* rewritten,
    /// which is covered by the migration-specific tests.
    #[tokio::test]
    async fn test_load_is_a_noop_write_when_no_legacy_fields_present() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("clean.json");

        let clean = json!({
            "version": 17,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "archived_boards": [],
                "boards": [], "columns": [], "cards": [], "archived_cards": [], "sprints": [],
                "prefixes": [],
                "graph": {
                    "spawns": { "edges": [] },
                    "blocks": { "edges": [] },
                    "relates": { "edges": [] }
                }
            }
        });
        let original_bytes = serde_json::to_vec_pretty(&clean).unwrap();
        tokio::fs::write(&file_path, &original_bytes).await.unwrap();

        let store = JsonFileStore::new(&file_path);
        let _ = store.load().await.unwrap();

        let after_bytes = tokio::fs::read(&file_path).await.unwrap();
        assert_eq!(
            original_bytes, after_bytes,
            "loading a clean file must not rewrite it"
        );
    }

    /// Regression test for KAN-504 migration round-trip bug.
    ///
    /// The V6 split-graph migration removes the `edge_type` key from each
    /// migrated edge (it lives implicitly in the sub-graph the edge is
    /// routed to). The post-migration file must still load through the
    /// `LegacyEdge<()>` deserialiser — otherwise we produce files that can't be
    /// loaded by the very code that wrote them. Was missed by the unit
    /// tests on the migration's in-memory output, which never round-
    /// tripped through `LegacyEdge::deserialize`.
    #[tokio::test]
    async fn test_v3_file_with_edges_round_trips_through_migration_and_load() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("v3_with_edges.json");

        let v3_content = json!({
            "version": 3,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [],
                "columns": [],
                "cards": [],
                "archived_cards": [],
                "sprints": [],
                "graph": {
                    "cards": {
                        "edges": [
                            {
                                "source": "11111111-1111-1111-1111-111111111111",
                                "target": "22222222-2222-2222-2222-222222222222",
                                "edge_type": "ParentOf",
                                "direction": "Directed",
                                "weight": null,
                                "created_at": "2024-01-01T00:00:00Z",
                                "archived_at": null
                            },
                            {
                                "source": "33333333-3333-3333-3333-333333333333",
                                "target": "44444444-4444-4444-4444-444444444444",
                                "edge_type": "Blocks",
                                "direction": "Directed",
                                "weight": null,
                                "created_at": "2024-01-01T00:00:00Z",
                                "archived_at": null
                            },
                            {
                                "source": "55555555-5555-5555-5555-555555555555",
                                "target": "66666666-6666-6666-6666-666666666666",
                                "edge_type": "RelatesTo",
                                "direction": "Bidirectional",
                                "weight": null,
                                "created_at": "2024-01-01T00:00:00Z",
                                "archived_at": null
                            }
                        ]
                    }
                }
            }
        });
        tokio::fs::write(&file_path, v3_content.to_string())
            .await
            .unwrap();

        // Trigger migration on first load.
        let store = JsonFileStore::new(&file_path);
        store
            .load()
            .await
            .expect("first load (migration) must succeed");

        // Re-open and load again — this exercises the
        // `LegacyEdge::deserialize` path on the post-migration file shape.
        let store2 = JsonFileStore::new(&file_path);
        let (snapshot, _meta) = store2
            .load()
            .await
            .expect("re-load of migrated file must succeed");

        // Decode the snapshot bytes through the full domain stack —
        // this is what kanban-service does at startup, and it's where
        // the bug actually triggers because LegacyEdge<()>::deserialize
        // requires the `edge_type` field by default.
        use kanban_persistence::snapshot_from_json_bytes;
        let domain_snapshot = snapshot_from_json_bytes(&snapshot.data)
            .expect("snapshot must deserialize through the full domain stack after migration");
        assert_eq!(domain_snapshot.graph.spawns_edges().len(), 1);
        assert_eq!(domain_snapshot.graph.blocks_edges().len(), 1);
        assert_eq!(domain_snapshot.graph.relates_edges().len(), 1);
    }

    #[tokio::test]
    async fn test_v3_file_loads_correctly() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("v3.json");

        let v3_content = json!({
            "version": 3,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [],
                "columns": [],
                "cards": [],
                "archived_cards": [],
                "sprints": [],
                "graph": { "cards": { "edges": [] } }
            }
        });
        tokio::fs::write(&file_path, v3_content.to_string())
            .await
            .unwrap();

        let store = JsonFileStore::new(&file_path);
        let (snapshot, _meta) = store.load().await.unwrap();
        let loaded: serde_json::Value = serde_json::from_slice(&snapshot.data).unwrap();
        assert!(loaded["boards"].is_array());
    }

    #[test]
    fn test_migrate_v1_to_v2_sync_produces_valid_v2_and_leaves_no_artifacts() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        let v1_content = json!({ "boards": [] });
        std::fs::write(&path, v1_content.to_string()).unwrap();

        let store = JsonFileStore::new(&path);
        let result = store.load_sync().unwrap();
        assert!(result.is_some(), "load_sync must return a snapshot");

        let on_disk: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let version = on_disk.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
        assert!(
            version >= 2,
            "file on disk must be V2+ envelope after migration"
        );

        let backup_path = path.with_extension("v1.backup");
        assert!(
            !backup_path.exists(),
            ".v1.backup must not remain after successful migration"
        );

        let tmp_path = path.with_extension("tmp");
        assert!(
            !tmp_path.exists(),
            ".tmp must not remain after successful migration"
        );
    }

    /// KAN-650: the sync migration path must mirror the async orchestrator's
    /// `.v{N}.backup` behaviour. A V6 file with both `parent_child` AND
    /// `spawns` buckets is the canonical mid-V6 failure case (the v6→v7
    /// rename refuses it). Without a pre-chain backup, the user has nothing
    /// to roll back to. With one, the broken envelope on disk plus the
    /// `.v6.backup` together reconstruct the original state.
    #[test]
    fn test_load_sync_v6_to_v7_preserves_v6_backup_on_failure() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v6_ambiguous_sync.json");
        let v6 = json!({
            "version": 6,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [], "columns": [], "cards": [], "archived_cards": [], "sprints": [],
                "graph": {
                    "parent_child": { "edges": [{
                        "source": "11111111-1111-1111-1111-111111111111",
                        "target": "22222222-2222-2222-2222-222222222222",
                        "created_at": "2024-01-01T00:00:00Z",
                        "archived_at": null
                    }]},
                    "spawns": { "edges": [{
                        "source": "33333333-3333-3333-3333-333333333333",
                        "target": "44444444-4444-4444-4444-444444444444",
                        "created_at": "2024-01-01T00:00:00Z",
                        "archived_at": null
                    }]},
                    "blocks": { "edges": [] },
                    "relates": { "edges": [] }
                }
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&v6).unwrap()).unwrap();

        let store = JsonFileStore::new(&path);
        let err = store
            .load_sync()
            .expect_err("load_sync must refuse a V6 envelope carrying both bucket keys");
        let msg = err.to_string();
        assert!(
            msg.contains("parent_child") && msg.contains("spawns"),
            "diagnostic should name both colliding keys; got: {msg}"
        );

        assert!(
            path.with_extension("v6.backup").exists(),
            ".v6.backup must be preserved when the V6→V7 sync step fails so the user can recover"
        );
    }

    /// KAN-650: successful V6→V8 sync migration must clean up its
    /// `.v6.backup` once the chain completes. Mirrors the async
    /// `test_migrate_v6_to_v7_renames_parent_child_and_writes_backup`
    /// assertion that the backup is removed on success.
    #[test]
    fn test_load_sync_v6_to_v7_cleans_up_v6_backup_on_success() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v6_clean_sync.json");
        let v6 = json!({
            "version": 6,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [], "columns": [], "cards": [], "archived_cards": [], "sprints": [],
                "graph": {
                    "parent_child": { "edges": [] },
                    "blocks": { "edges": [] },
                    "relates": { "edges": [] }
                }
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&v6).unwrap()).unwrap();

        let store = JsonFileStore::new(&path);
        let _ = store.load_sync().unwrap().expect("file exists");

        let after: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(after["version"], 17);

        assert!(
            !path.with_extension("v6.backup").exists(),
            ".v6.backup must be removed after successful V6→V8 sync migration"
        );
    }

    /// KAN-650: V5 files go through split_graph then v6→v7. The backup
    /// keyed to the *source* version is `.v5.backup`, written before
    /// the destructive chain runs and removed on success.
    #[test]
    fn test_load_sync_v5_to_v7_cleans_up_v5_backup_on_success() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v5_clean_sync.json");
        let v5 = json!({
            "version": 5,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [], "columns": [], "cards": [], "archived_cards": [], "sprints": [],
                "graph": { "cards": { "edges": [] } }
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&v5).unwrap()).unwrap();

        let store = JsonFileStore::new(&path);
        let _ = store.load_sync().unwrap().expect("file exists");

        let after: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(after["version"], 17);

        assert!(
            !path.with_extension("v5.backup").exists(),
            ".v5.backup must be removed after successful V5→V8 sync migration"
        );
    }

    /// KAN-650: V4 backup keyed to the source version.
    #[test]
    fn test_load_sync_v4_to_v7_cleans_up_v4_backup_on_success() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v4_clean_sync.json");
        let v4 = json!({
            "version": 4,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [], "columns": [], "cards": [], "archived_cards": [], "sprints": [],
                "graph": { "cards": { "edges": [] } }
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&v4).unwrap()).unwrap();

        let store = JsonFileStore::new(&path);
        let _ = store.load_sync().unwrap().expect("file exists");

        let after: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(after["version"], 17);

        assert!(
            !path.with_extension("v4.backup").exists(),
            ".v4.backup must be removed after successful V4→V8 sync migration"
        );
    }

    /// KAN-650: V3 backup keyed to the source version.
    #[test]
    fn test_load_sync_v3_to_v7_cleans_up_v3_backup_on_success() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v3_clean_sync.json");
        let v3 = json!({
            "version": 3,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [], "columns": [], "cards": [], "archived_cards": [], "sprints": [],
                "graph": { "cards": { "edges": [] } }
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&v3).unwrap()).unwrap();

        let store = JsonFileStore::new(&path);
        let _ = store.load_sync().unwrap().expect("file exists");

        let after: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(after["version"], 17);

        assert!(
            !path.with_extension("v3.backup").exists(),
            ".v3.backup must be removed after successful V3→V8 sync migration"
        );
    }

    /// KAN-660: V2 sources are now covered by the outer pre-V7 backup wrap.
    /// The wrap takes the backup BEFORE migrate_v2_to_v3_sync runs, so the
    /// V2 original is captured. On successful V2→V8 the wrap cleans it up.
    /// (No paired failure-preservation test for V2: the V2 envelope shape
    /// can't cleanly inject the V6 both-keys ambiguity that drives the
    /// existing V6 failure test, and the outer-wrap failure-handling code
    /// path is the same `match (result, backup_path)` block already
    /// exercised by `test_load_sync_v6_to_v7_preserves_v6_backup_on_failure`.)
    #[test]
    fn test_load_sync_v2_to_v7_cleans_up_v2_backup_on_success() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v2_clean_sync.json");
        let v2 = json!({
            "version": 2,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [], "columns": [], "cards": [], "archived_cards": [], "sprints": []
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&v2).unwrap()).unwrap();

        let store = JsonFileStore::new(&path);
        let _ = store.load_sync().unwrap().expect("file exists");

        let after: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(after["version"], 17);

        assert!(
            !path.with_extension("v2.backup").exists(),
            ".v2.backup must be removed after successful V2→V8 sync migration"
        );
    }

    /// KAN-660: V1 sources are now covered by the outer pre-V7 backup wrap.
    /// The .v1.backup is taken BEFORE migrate_v1_to_v2_sync runs and only
    /// cleaned up after the entire V1→V8 chain succeeds — not after the
    /// V1→V2 step like the pre-KAN-660 per-step mechanism did. This means
    /// a mid-chain failure (e.g. during split_graph or v6_to_v7_rename)
    /// preserves the V1 original instead of losing it after V1→V2.
    #[test]
    fn test_load_sync_v1_to_v7_cleans_up_v1_backup_on_success() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v1_clean_sync.json");
        let v1 = json!({
            "boards": [],
            "columns": [],
            "cards": []
        });
        std::fs::write(&path, v1.to_string()).unwrap();

        let store = JsonFileStore::new(&path);
        let _ = store.load_sync().unwrap().expect("file exists");

        let after: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(after["version"], 17);

        assert!(
            !path.with_extension("v1.backup").exists(),
            ".v1.backup must be removed after successful V1→V8 sync migration"
        );
    }

    fn fully_populated_card() -> kanban_domain::Card {
        use kanban_domain::{CardPriority, CardRecord, CardStatus, SprintLog};
        use uuid::Uuid;

        let sprint_id = Uuid::new_v4();
        let record = CardRecord {
            id: Uuid::new_v4(),
            column_id: Uuid::new_v4(),
            board_id: Uuid::new_v4(),
            title: "Done card".to_string(),
            description: Some("finished".to_string()),
            priority: CardPriority::High,
            status: CardStatus::Done,
            position: 7,
            due_date: Some("2024-05-05T00:00:00Z".parse().unwrap()),
            points: Some(3),
            card_number: 42,
            sprint_id: Some(sprint_id),
            created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
            updated_at: "2024-02-02T00:00:00Z".parse().unwrap(),
            completed_at: Some("2024-03-03T00:00:00Z".parse().unwrap()),
            sprint_logs: vec![
                SprintLog {
                    sprint_id,
                    sprint_number: 1,
                    sprint_name: Some("Sprint 1".to_string()),
                    started_at: "2024-01-10T00:00:00Z".parse().unwrap(),
                    ended_at: Some("2024-01-20T00:00:00Z".parse().unwrap()),
                    status: "Completed".to_string(),
                },
                SprintLog {
                    sprint_id,
                    sprint_number: 2,
                    sprint_name: None,
                    started_at: "2024-02-01T00:00:00Z".parse().unwrap(),
                    ended_at: None,
                    status: "Active".to_string(),
                },
            ],
            prefix: String::new(),
        };
        kanban_domain::Card::reconstitute(record).unwrap()
    }

    #[tokio::test]
    async fn test_json_card_round_trip_preserves_all_fields() {
        use kanban_persistence::{snapshot_from_json_bytes, snapshot_to_json_bytes};

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("cards.json");
        let store = JsonFileStore::new(&file_path);

        let card = fully_populated_card();
        let snapshot = kanban_domain::Snapshot::from_data(
            vec![],
            vec![],
            vec![card.clone()],
            vec![],
            vec![],
            kanban_domain::DependencyGraph::new(),
        );
        let store_snapshot = StoreSnapshot {
            data: snapshot_to_json_bytes(&snapshot).unwrap(),
            metadata: PersistenceMetadata::new(store.instance_id()),
        };
        store.save(store_snapshot).await.unwrap();

        let (loaded, _meta) = store.load().await.unwrap();
        let domain = snapshot_from_json_bytes(&loaded.data).unwrap();
        assert_eq!(domain.cards.len(), 1);
        assert_eq!(domain.cards[0], card);
    }

    #[tokio::test]
    async fn test_json_card_round_trip_preserves_sprint_logs_verbatim() {
        use kanban_persistence::{snapshot_from_json_bytes, snapshot_to_json_bytes};

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("card_logs.json");
        let store = JsonFileStore::new(&file_path);

        let card = fully_populated_card();
        let snapshot = kanban_domain::Snapshot::from_data(
            vec![],
            vec![],
            vec![card.clone()],
            vec![],
            vec![],
            kanban_domain::DependencyGraph::new(),
        );
        let store_snapshot = StoreSnapshot {
            data: snapshot_to_json_bytes(&snapshot).unwrap(),
            metadata: PersistenceMetadata::new(store.instance_id()),
        };
        store.save(store_snapshot).await.unwrap();

        let (loaded, _meta) = store.load().await.unwrap();
        let domain = snapshot_from_json_bytes(&loaded.data).unwrap();
        assert_eq!(domain.cards[0].sprint_logs, card.sprint_logs);
        assert_eq!(domain.cards[0].sprint_logs.len(), 2);
        assert_eq!(domain.cards[0].sprint_logs[1].ended_at, None);
    }

    #[tokio::test]
    async fn test_json_archived_card_round_trip_preserves_card() {
        use kanban_persistence::{snapshot_from_json_bytes, snapshot_to_json_bytes};
        use uuid::Uuid;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("archived.json");
        let store = JsonFileStore::new(&file_path);

        let card = fully_populated_card();
        // Reference-marker model: the card stays LIVE in `.cards`; `archived_cards`
        // carries a pure marker referencing it by `entity_id`.
        let archived = kanban_domain::ArchivedCard::new(card.id, Uuid::new_v4());
        let snapshot = kanban_domain::Snapshot::from_data(
            vec![],
            vec![],
            vec![card.clone()],
            vec![archived],
            vec![],
            kanban_domain::DependencyGraph::new(),
        );
        let store_snapshot = StoreSnapshot {
            data: snapshot_to_json_bytes(&snapshot).unwrap(),
            metadata: PersistenceMetadata::new(store.instance_id()),
        };
        store.save(store_snapshot).await.unwrap();

        let (loaded, _meta) = store.load().await.unwrap();
        let domain = snapshot_from_json_bytes(&loaded.data).unwrap();
        // The live card round-trips verbatim under `.cards`; the marker round-trips
        // under `.archived_cards` referencing it by id (no embedded entity).
        assert_eq!(domain.cards.len(), 1);
        assert_eq!(domain.cards[0], card);
        assert_eq!(domain.archived_cards.len(), 1);
        assert_eq!(domain.archived_cards[0].entity_id, card.id);
        assert_eq!(domain.archived_cards[0], archived);
    }
}
