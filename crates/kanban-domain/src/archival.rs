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
use serde::{Deserialize, Deserializer, Serialize, Serializer};
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

/// A live entity that can be archived: exposes its stable id and its own serde
/// bridge. Live entities (`Board`/`Card`) have no `Deserialize` — they route
/// through a record type for migration — so the bridge is explicit. Implementing
/// this trait is the ONE thing a new archivable entity must do; the wrapper,
/// metadata, and lifecycle come for free.
pub trait ArchivableEntity: Sized {
    fn entity_id(&self) -> Uuid;

    fn serialize_entity<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error>;

    fn deserialize_entity<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error>;
}

/// serde bridge that dispatches a generic entity field through its
/// [`ArchivableEntity`] impl, so `Archived<T, _>` can `#[serde(with)]` a `T`
/// it cannot name at derive time.
pub mod entity_serde {
    use super::ArchivableEntity;
    use serde::{Deserializer, Serializer};

    pub fn serialize<T: ArchivableEntity, S: Serializer>(t: &T, s: S) -> Result<S::Ok, S::Error> {
        t.serialize_entity(s)
    }

    pub fn deserialize<'de, T: ArchivableEntity, D: Deserializer<'de>>(
        d: D,
    ) -> Result<T, D::Error> {
        T::deserialize_entity(d)
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
///
/// The entity serializes under the `entity` key; `alias = "card"` keeps
/// already-shipped `archived_cards` blobs (which used a `card` key) loadable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "T: ArchivableEntity, C: Serialize",
    deserialize = "T: ArchivableEntity, C: Deserialize<'de>"
))]
pub struct Archived<T, C = NoContext> {
    #[serde(with = "entity_serde", alias = "card")]
    pub entity: T,
    #[serde(flatten)]
    pub metadata: ArchiveMetadata,
    #[serde(flatten)]
    pub context: C,
}

impl<T: ArchivableEntity, C> Archived<T, C> {
    pub fn with_context(entity: T, context: C, metadata: ArchiveMetadata) -> Self {
        Self {
            entity,
            context,
            metadata,
        }
    }
}

impl<T: ArchivableEntity> Archived<T, NoContext> {
    /// Archive a scoping-root entity now (no restore context).
    pub fn now(entity: T) -> Self {
        Self {
            entity,
            context: NoContext {},
            metadata: ArchiveMetadata::now(),
        }
    }

    /// Archive a scoping-root entity at an explicit time.
    pub fn at(entity: T, archived_at: DateTime<Utc>) -> Self {
        Self {
            entity,
            context: NoContext {},
            metadata: ArchiveMetadata::at(archived_at),
        }
    }

    /// Restore: unwrap the live entity verbatim.
    pub fn into_entity(self) -> T {
        self.entity
    }
}

impl<T: ArchivableEntity, C> ArchivedEntity for Archived<T, C> {
    fn entity_id(&self) -> Uuid {
        self.entity.entity_id()
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
}
