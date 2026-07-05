//! Bounded shared archival abstraction.
//!
//! Scope is deliberately minimal (validated by two execution spikes): the only
//! genuinely cross-cutting pieces are the archived-metadata envelope and a thin
//! trait exposing an archived record's stable key + metadata. The lifecycle
//! (archive / restore / permanent-delete) and the entity-specific restore
//! context stay specialized on the concrete types and the command tier — a
//! store-generic `ArchiveCollection` trait was found to be dead abstraction (no
//! backend would implement it) and is intentionally NOT provided.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Shared archival metadata envelope. Common to every archived entity; the
/// entity-specific restore context is NOT stored here (it stays on the concrete
/// archived type, e.g. `ArchivedCard::original_column_id`). Kept a one-field
/// struct on purpose: it is the seam where a future `archived_by`/reason would
/// live without touching call sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveMetadata {
    pub archived_at: DateTime<Utc>,
}

impl ArchiveMetadata {
    pub fn now() -> Self {
        Self {
            archived_at: Utc::now(),
        }
    }

    pub fn at(archived_at: DateTime<Utc>) -> Self {
        Self { archived_at }
    }
}

/// A discrete archived record: knows the live entity's stable id (its archive
/// key) and its shared metadata. Payload beyond this stays specialized on the
/// concrete type — the trait covers the envelope + key, not the payload.
pub trait ArchivedEntity {
    /// Stable id of the live entity this record archives (card id / board id).
    fn entity_id(&self) -> Uuid;

    fn metadata(&self) -> ArchiveMetadata;

    fn archived_at(&self) -> DateTime<Utc> {
        self.metadata().archived_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_metadata_now_sets_recent_timestamp() {
        let before = Utc::now();
        let meta = ArchiveMetadata::now();
        assert!(meta.archived_at >= before);
        assert!(meta.archived_at <= Utc::now());
    }

    struct Dummy {
        id: Uuid,
        at: DateTime<Utc>,
    }

    impl ArchivedEntity for Dummy {
        fn entity_id(&self) -> Uuid {
            self.id
        }
        fn metadata(&self) -> ArchiveMetadata {
            ArchiveMetadata::at(self.at)
        }
    }

    #[test]
    fn test_archived_entity_exposes_entity_id_and_archived_at() {
        let id = Uuid::new_v4();
        let at = Utc::now();
        let d = Dummy { id, at };
        assert_eq!(d.entity_id(), id);
        assert_eq!(d.metadata().archived_at, at);
        // The default `archived_at()` delegates to `metadata()`.
        assert_eq!(d.archived_at(), at);
    }
}
