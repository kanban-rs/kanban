//! Shared archival abstraction.
//!
//! Archived records are modeled generically as [`Archived<C>`]: a pure marker
//! recording that a live entity (referenced by `entity_id`) was archived, plus
//! its shared [`ArchiveMetadata`] envelope and an entity-specific restore
//! context `C` ([`NoContext`] for scoping roots). The entity is NEVER embedded;
//! it stays live in its own collection. `ArchivedCard`, `ArchivedBoard`, and
//! any future archived type are aliases of it, so archiving a new entity means
//! choosing its restore context — the marker, the metadata envelope, and the
//! archive/restore lifecycle come for free.
//!
//! Deliberately bounded to the DOMAIN shape. A store-generic
//! `ArchiveCollection` trait was found (across two execution spikes) to be dead
//! abstraction — no backend would implement it — so persistence stays
//! entity-specific (each backend discerns the type via its own tables/queries)
//! and that trait is intentionally NOT provided.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Shared archival metadata envelope. Common to every archived entity; the
/// entity-specific restore context is NOT stored here (it stays on the concrete
/// archived type's context, e.g. `CardRestoreContext::board_id`). Kept a one-field
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

/// Empty restore context for scoping-root entities (e.g. a board) that need no
/// "where did it come from" data. Flattens to nothing on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NoContext {}

/// A generic archived record: any live entity `T` plus its shared
/// [`ArchiveMetadata`] envelope and an entity-specific restore context `C`
/// (`NoContext` for roots). The reusable archival shape — `ArchivedCard`,
/// `ArchivedBoard`, and any future one are aliases. Storage stays
/// entity-specific (backends discern the type); this is only the domain shape.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Archived<C = NoContext> {
    /// Stable id of the live entity this record archives (card id / board id).
    /// The entity itself is NOT embedded — it stays live in its own collection
    /// (reference-marker model); this is a pure marker.
    pub entity_id: Uuid,
    #[serde(flatten)]
    pub metadata: ArchiveMetadata,
    #[serde(flatten)]
    pub context: C,
}

impl<C> Archived<C> {
    pub fn with_context(entity_id: Uuid, context: C, metadata: ArchiveMetadata) -> Self {
        Self {
            entity_id,
            context,
            metadata,
        }
    }
}

impl Archived<NoContext> {
    /// Archive a scoping-root entity now (no restore context).
    pub fn now(entity_id: Uuid) -> Self {
        Self {
            entity_id,
            context: NoContext {},
            metadata: ArchiveMetadata::now(),
        }
    }

    /// Archive a scoping-root entity at an explicit time.
    pub fn at(entity_id: Uuid, archived_at: DateTime<Utc>) -> Self {
        Self {
            entity_id,
            context: NoContext {},
            metadata: ArchiveMetadata::at(archived_at),
        }
    }
}

impl<C> ArchivedEntity for Archived<C> {
    fn entity_id(&self) -> Uuid {
        self.entity_id
    }

    fn metadata(&self) -> ArchiveMetadata {
        self.metadata
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

    #[test]
    fn test_archived_now_sets_recent_metadata_and_no_context() {
        let id = Uuid::new_v4();
        let before = Utc::now();
        let a = Archived::now(id);
        assert_eq!(a.entity_id, id);
        assert_eq!(a.context, NoContext {});
        assert!(a.metadata.archived_at >= before && a.metadata.archived_at <= Utc::now());
    }

    #[test]
    fn test_archived_at_uses_injected_time_and_exposes_entity_id() {
        let id = Uuid::new_v4();
        let ts = Utc::now();
        let a = Archived::at(id, ts);
        assert_eq!(a.metadata.archived_at, ts);
        assert_eq!(a.entity_id, id);
    }

    #[test]
    fn test_archived_root_blanket_archived_entity_impl() {
        let id = Uuid::new_v4();
        let ts = Utc::now();
        let a = Archived::at(id, ts);
        assert_eq!(a.entity_id(), id);
        assert_eq!(a.archived_at(), ts);
    }

    #[test]
    fn test_archived_nocontext_round_trips_json_referenced_flat_metadata() {
        let ts = Utc::now();
        let a = Archived::at(Uuid::new_v4(), ts);
        let v = serde_json::to_value(a).unwrap();
        // Entity referenced by id (never embedded), archived_at flat, no context.
        assert!(
            v.get("entity").is_none(),
            "entity is referenced, not embedded"
        );
        assert!(
            v.get("entity_id").is_some(),
            "entity_id is present, got: {v}"
        );
        assert!(v.get("archived_at").is_some(), "metadata flattens");
        assert!(v.get("context").is_none(), "NoContext flattens to nothing");
        let back: Archived<NoContext> = serde_json::from_value(v).unwrap();
        assert_eq!(back, a);
    }
}
