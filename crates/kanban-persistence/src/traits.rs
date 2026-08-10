use crate::PersistenceResult;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Metadata for persistence operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceMetadata {
    /// ID of the instance that performed the save
    pub instance_id: Uuid,
    /// When this data was saved
    pub saved_at: DateTime<Utc>,
    /// Semver of the kanban that wrote this file. `None` on legacy files
    /// written before the stamp was introduced.
    #[serde(default)]
    pub writer_version: Option<String>,
    /// Git commit of the kanban that wrote this file. `None` on legacy files
    /// or builds without a git checkout (`"unknown"` is also possible).
    #[serde(default)]
    pub writer_commit: Option<String>,
    /// Format version of the loaded file as the backend understands it
    /// (JSON envelope version, SQLite schema_version). `None` when not
    /// populated by the backend. Display-only — backends still own version
    /// negotiation.
    #[serde(default, skip_serializing)]
    pub format_version: Option<u32>,
}

impl PersistenceMetadata {
    pub fn new(instance_id: Uuid) -> Self {
        Self {
            instance_id,
            saved_at: Utc::now(),
            writer_version: None,
            writer_commit: None,
            format_version: None,
        }
    }

    /// Stamp the writer identity onto the metadata. Backends call this on the
    /// save path so the saved file remembers which kanban produced it.
    pub fn with_writer_stamp(
        mut self,
        version: impl Into<String>,
        commit: impl Into<String>,
    ) -> Self {
        self.writer_version = Some(version.into());
        self.writer_commit = Some(commit.into());
        self
    }
}

/// Point-in-time snapshot of all data that needs to be persisted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreSnapshot {
    /// Raw JSON bytes representing all boards, columns, cards, etc.
    pub data: Vec<u8>,
    /// Metadata about this snapshot
    pub metadata: PersistenceMetadata,
}

/// Events that can be emitted during persistence operations
#[derive(Debug, Clone)]
pub enum PersistenceEvent {
    /// Data was successfully saved
    Saved(PersistenceMetadata),
    /// External changes were detected
    ExternalChangeDetected {
        path: PathBuf,
        saved_at: DateTime<Utc>,
    },
    /// A conflict occurred (our changes vs external changes)
    ConflictDetected { reason: String },
    /// An error occurred during persistence
    Error(String),
}

/// Trait for abstract storage operations
/// Implementations handle different backend storage (file, database, etc.)
#[async_trait]
pub trait PersistenceStore: Send + Sync {
    /// Save a snapshot to the store
    async fn save(&self, snapshot: StoreSnapshot) -> PersistenceResult<PersistenceMetadata>;

    /// Load the current snapshot from the store
    async fn load(&self) -> PersistenceResult<(StoreSnapshot, PersistenceMetadata)>;

    /// Check if the store file exists
    async fn exists(&self) -> bool;

    /// Get the path to the store file
    fn path(&self) -> &Path;

    /// Get the unique instance ID for this store
    fn instance_id(&self) -> uuid::Uuid;

    /// Load the store synchronously (no async runtime required).
    /// Returns `Ok(None)` when the backing file does not exist.
    ///
    /// The default implementation returns an error; backends that support
    /// synchronous loading (e.g. `JsonFileStore`) override this.
    #[allow(clippy::type_complexity)]
    fn load_sync(&self) -> PersistenceResult<Option<(StoreSnapshot, PersistenceMetadata)>> {
        Err(crate::PersistenceError::Unsupported(
            "load_sync not supported by this backend".into(),
        ))
    }

    /// Drain any open connections / file handles before the backing file is
    /// unlinked. Required on Windows: the OS refuses to delete files that
    /// still have live handles, and async resources (e.g. an `sqlx` pool)
    /// outlive synchronous `Drop` because the runtime needs time to close
    /// each connection.
    ///
    /// The default is a no-op; backends with long-lived handles (e.g.
    /// `SqliteStore`) override this.
    async fn close(&self) {}
}

/// Trait for detecting changes to the storage file
/// Used for multi-instance coordination
#[async_trait]
pub trait ChangeDetector: Send + Sync {
    /// Start watching the file for changes
    async fn start_watching(&self, path: PathBuf) -> PersistenceResult<()>;

    /// Stop watching the file
    async fn stop_watching(&self) -> PersistenceResult<()>;

    /// Subscribe to change events
    /// Returns a broadcast receiver that yields `ChangeEvent` when the file changes
    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<ChangeEvent>;

    /// Check if currently watching
    fn is_watching(&self) -> bool;
}

/// Event indicating a change to the watched file
#[derive(Debug, Clone)]
pub struct ChangeEvent {
    /// Path to the file that changed
    pub path: PathBuf,
    /// When the change was detected
    pub detected_at: DateTime<Utc>,
}

/// Trait for serialization/deserialization strategies
/// Allows swapping JSON for binary formats, databases, etc.
pub trait Serializer<T: Send + Sync>: Send + Sync {
    /// Serialize data to bytes
    fn serialize(&self, data: &T) -> PersistenceResult<Vec<u8>>;

    /// Deserialize data from bytes
    fn deserialize(&self, bytes: &[u8]) -> PersistenceResult<T>;
}

/// Format versions for migration tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FormatVersion {
    V1,
    V2,
    V3,
    V4,
    V5,
    /// V6 splits the dependency graph from a single edge-type-tagged list
    /// (`graph.cards.edges`) into three sub-graphs keyed by edge kind
    /// (`graph.parent_child`, `graph.blocks`, `graph.relates`).
    V6,
    /// V7 renames the spawns sub-graph bucket from `parent_child` to
    /// `spawns` so the wire format matches the `SpawnsEdge` struct,
    /// the `spawns_edges()` accessor, and the SQLite `spawns_edges`
    /// table. Pure key rename — edge contents are unchanged.
    V7,
    /// V8 promotes archived cards to a first-class discrete collection:
    /// each `archived_cards` entry carries its own `board_id`, backfilled
    /// from `original_column_id`→column→board for historical files (nil
    /// when the original column no longer resolves), and is guaranteed not
    /// to be duplicated inside the `cards` array.
    V8,
    /// V9 marks the format as archived-board-capable. The `archived_boards`
    /// collection is additive (serde-default), but the bump makes a pre-V9
    /// binary reject the file rather than silently drop archived boards on save.
    V9,
    /// V10 collapses the archival wrapper into a PURE MARKER (F3b). Historical
    /// `archived_cards` / `archived_boards` entries EMBEDDED their entity (under
    /// `entity`, legacy alias `card`/`board`) and did not carry it in the live
    /// `cards`/`boards` arrays. V10 lifts each embedded entity into the live
    /// collection and rewrites each wrapper as a reference marker
    /// (`{ entity_id, archived_at, board_id? }`), dropping the retired
    /// `original_column_id`/`original_position` restore context.
    V10,
    /// V11 backfills `board_id` onto each historical `cards` entry that lacks
    /// it, mirroring V8's `archived_cards.board_id` backfill one layer
    /// earlier: `Card.board_id` is now a durable field set at creation and
    /// kept in sync on every move (KAN-963), independent of `column_id` --
    /// derived here from `column_id`→column→`board_id` for files predating
    /// the field (nil when the column no longer resolves).
    V11,
    /// V12 makes `Board.completion_column_ids` durable: each board's legacy
    /// `completion_column_id` key is removed and replaced by
    /// `completion_column_ids`, a one-element list backfilled from that
    /// legacy id when it still names a live column of the board, otherwise
    /// from the board's last column ordered by `position`, then
    /// `created_at`, then `id` — the same deterministic ordering
    /// `sorted_board_columns` uses everywhere else, replacing the
    /// non-deterministic tie-break of the old runtime fallback.
    V12,
}

impl FormatVersion {
    /// The highest format version this binary can read or produce.
    pub const MAX: Self = Self::V12;

    pub fn as_u32(self) -> u32 {
        match self {
            Self::V1 => 1,
            Self::V2 => 2,
            Self::V3 => 3,
            Self::V4 => 4,
            Self::V5 => 5,
            Self::V6 => 6,
            Self::V7 => 7,
            Self::V8 => 8,
            Self::V9 => 9,
            Self::V10 => 10,
            Self::V11 => 11,
            Self::V12 => 12,
        }
    }

    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            1 => Some(Self::V1),
            2 => Some(Self::V2),
            3 => Some(Self::V3),
            4 => Some(Self::V4),
            5 => Some(Self::V5),
            6 => Some(Self::V6),
            7 => Some(Self::V7),
            8 => Some(Self::V8),
            9 => Some(Self::V9),
            10 => Some(Self::V10),
            11 => Some(Self::V11),
            12 => Some(Self::V12),
            _ => None,
        }
    }
}

/// Trait for migration strategies between format versions
#[async_trait]
pub trait MigrationStrategy: Send + Sync {
    /// Detect the version of a file on disk
    async fn detect_version(&self, path: &Path) -> PersistenceResult<FormatVersion>;

    /// Migrate from one version to another
    /// Returns the path to the migrated file
    async fn migrate(
        &self,
        from: FormatVersion,
        to: FormatVersion,
        path: &Path,
    ) -> PersistenceResult<PathBuf>;
}

/// Trait for conflict resolution between local and external changes
pub trait ConflictResolver: Send + Sync {
    /// Determine whether local or external change wins
    /// Returns true if external changes should be used, false for local changes
    fn should_use_external(
        &self,
        local_metadata: &PersistenceMetadata,
        external_metadata: &PersistenceMetadata,
    ) -> bool;

    /// Get a human-readable description of the conflict resolution
    fn explain_resolution(
        &self,
        local_metadata: &PersistenceMetadata,
        external_metadata: &PersistenceMetadata,
    ) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_version_max_equals_v12() {
        assert_eq!(FormatVersion::MAX, FormatVersion::V12);
    }

    #[test]
    fn test_format_version_max_as_u32_matches_largest_variant() {
        assert_eq!(FormatVersion::MAX.as_u32(), 12);
    }

    #[test]
    fn test_from_u32_accepts_12_rejects_13() {
        assert_eq!(FormatVersion::from_u32(11), Some(FormatVersion::V11));
        assert_eq!(FormatVersion::from_u32(12), Some(FormatVersion::V12));
        assert_eq!(FormatVersion::from_u32(13), None);
    }

    #[test]
    fn test_default_metadata_has_no_writer_stamp() {
        let md = PersistenceMetadata::new(Uuid::nil());
        assert!(md.writer_version.is_none());
        assert!(md.writer_commit.is_none());
    }

    #[test]
    fn test_with_writer_stamp_populates_both_fields() {
        let md = PersistenceMetadata::new(Uuid::nil()).with_writer_stamp("0.6.0", "abc1234");
        assert_eq!(md.writer_version.as_deref(), Some("0.6.0"));
        assert_eq!(md.writer_commit.as_deref(), Some("abc1234"));
    }

    #[test]
    fn test_metadata_serde_round_trips_with_writer_stamp() {
        let md = PersistenceMetadata::new(Uuid::nil()).with_writer_stamp("0.6.0", "abc1234");
        let json = serde_json::to_string(&md).unwrap();
        let parsed: PersistenceMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.writer_version.as_deref(), Some("0.6.0"));
        assert_eq!(parsed.writer_commit.as_deref(), Some("abc1234"));
    }

    #[test]
    fn test_metadata_deserialize_legacy_envelope_missing_stamp_fields() {
        // Old files lack writer_version / writer_commit. They must still parse.
        let legacy = serde_json::json!({
            "instance_id": "550e8400-e29b-41d4-a716-446655440000",
            "saved_at": "2024-01-01T00:00:00Z"
        });
        let parsed: PersistenceMetadata = serde_json::from_value(legacy).unwrap();
        assert!(parsed.writer_version.is_none());
        assert!(parsed.writer_commit.is_none());
    }
}
