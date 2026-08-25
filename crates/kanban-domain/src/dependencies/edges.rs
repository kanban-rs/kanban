//! Per-kind edge structs.
//!
//! Each concrete edge kind embeds [`EdgeBase`] for the common fields
//! (endpoints, timestamps, archival state) and adds its own
//! kind-specific metadata. All three implement the [`Edge`] trait
//! from `kanban-core::graph`, which is what [`crate::DagGraph`] /
//! [`crate::UndirectedGraph`] are generic over.
//!
//! Adding a new edge kind means defining a new struct alongside
//! these and implementing `Edge` for it — no change to existing
//! structs. The graph machinery (DagGraph / UndirectedGraph /
//! EdgeStore) is fully generic and works with any type that
//! satisfies the trait, including types defined outside this crate.

use chrono::{DateTime, Utc};
use kanban_core::graph::{Edge, EdgeBase};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::edge_meta::{RelatesKind, Severity};

/// Edge representing a directed parent->child relationship in a DAG,
/// not a tree: a child may have more than one parent.
///
/// Lives in the `parent_child: DagGraph<SpawnsEdge>` sub-graph.
/// Carries no metadata today; future kind-specific fields (e.g. a
/// per-child position within sibling ordering) extend this struct
/// without affecting `BlocksEdge` / `RelatesEdge`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawnsEdge {
    #[serde(flatten)]
    pub base: EdgeBase,
}

impl SpawnsEdge {
    pub fn new(parent: Uuid, child: Uuid) -> Self {
        Self {
            base: EdgeBase::new(parent, child),
        }
    }
}

/// Edge representing a blocking relationship: source blocks target.
///
/// Lives in the `blocks: DagGraph<BlocksEdge>` sub-graph.
/// `severity` lets algorithms weight blockers (e.g. shortest
/// minimum-severity blocker path to ship X). Defaults to
/// [`Severity::Medium`] for migrated edges that pre-date this field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlocksEdge {
    #[serde(flatten)]
    pub base: EdgeBase,
    #[serde(default)]
    pub severity: Severity,
}

impl BlocksEdge {
    pub fn new(blocker: Uuid, blocked: Uuid, severity: Severity) -> Self {
        Self {
            base: EdgeBase::new(blocker, blocked),
            severity,
        }
    }
}

/// Edge representing an undirected "relates to" link.
///
/// Lives in the `relates: UndirectedGraph<RelatesEdge>` sub-graph.
/// `kind` distinguishes the sub-kind of relation (general,
/// duplicates, mentioned-in, ...). Defaults to
/// [`RelatesKind::General`] for migrated edges that pre-date this
/// field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelatesEdge {
    #[serde(flatten)]
    pub base: EdgeBase,
    #[serde(default)]
    pub kind: RelatesKind,
}

impl RelatesEdge {
    pub fn new(a: Uuid, b: Uuid, kind: RelatesKind) -> Self {
        Self {
            base: EdgeBase::new(a, b),
            kind,
        }
    }
}

// --- Edge trait impls ---

impl Edge for SpawnsEdge {
    type NodeId = Uuid;

    fn source(&self) -> Uuid {
        self.base.source
    }
    fn target(&self) -> Uuid {
        self.base.target
    }
    fn created_at(&self) -> DateTime<Utc> {
        self.base.created_at
    }
    fn archived_at(&self) -> Option<DateTime<Utc>> {
        self.base.archived_at
    }
    fn archive(&mut self) {
        self.base.archive();
    }
    fn unarchive(&mut self) {
        self.base.unarchive();
    }
    fn from_endpoints(source: Uuid, target: Uuid) -> Self {
        SpawnsEdge::new(source, target)
    }
}

impl Edge for BlocksEdge {
    type NodeId = Uuid;

    fn source(&self) -> Uuid {
        self.base.source
    }
    fn target(&self) -> Uuid {
        self.base.target
    }
    fn created_at(&self) -> DateTime<Utc> {
        self.base.created_at
    }
    fn archived_at(&self) -> Option<DateTime<Utc>> {
        self.base.archived_at
    }
    fn archive(&mut self) {
        self.base.archive();
    }
    fn unarchive(&mut self) {
        self.base.unarchive();
    }
    fn from_endpoints(source: Uuid, target: Uuid) -> Self {
        BlocksEdge::new(source, target, Severity::default())
    }
}

impl Edge for RelatesEdge {
    type NodeId = Uuid;

    fn source(&self) -> Uuid {
        self.base.source
    }
    fn target(&self) -> Uuid {
        self.base.target
    }
    fn created_at(&self) -> DateTime<Utc> {
        self.base.created_at
    }
    fn archived_at(&self) -> Option<DateTime<Utc>> {
        self.base.archived_at
    }
    fn archive(&mut self) {
        self.base.archive();
    }
    fn unarchive(&mut self) {
        self.base.unarchive();
    }
    fn from_endpoints(source: Uuid, target: Uuid) -> Self {
        RelatesEdge::new(source, target, RelatesKind::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawns_edge_new_has_no_metadata() {
        let parent = Uuid::new_v4();
        let child = Uuid::new_v4();
        let e = SpawnsEdge::new(parent, child);
        assert_eq!(e.source(), parent);
        assert_eq!(e.target(), child);
        assert!(e.is_active());
    }

    #[test]
    fn test_blocks_edge_carries_severity() {
        let blocker = Uuid::new_v4();
        let blocked = Uuid::new_v4();
        let e = BlocksEdge::new(blocker, blocked, Severity::High);
        assert_eq!(e.source(), blocker);
        assert_eq!(e.severity, Severity::High);
    }

    #[test]
    fn test_relates_edge_carries_kind() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let e = RelatesEdge::new(a, b, RelatesKind::Duplicates);
        assert_eq!(e.kind, RelatesKind::Duplicates);
    }

    #[test]
    fn test_archive_unarchive_through_edge_trait_round_trips() {
        let mut e = BlocksEdge::new(Uuid::new_v4(), Uuid::new_v4(), Severity::Low);
        assert!(e.is_active());
        e.archive();
        assert!(e.is_archived());
        e.unarchive();
        assert!(e.is_active());
    }

    #[test]
    fn test_per_kind_structs_are_object_safe_as_dyn_edge() {
        // Cross-kind algorithms see the uniform &dyn Edge surface
        // without knowing concrete metadata. If a future trait method
        // takes `Self`, object safety breaks and this test fails to
        // compile.
        //
        // The `Edge` trait now carries an associated `NodeId`, so the
        // `dyn` view binds it explicitly. For the kanban domain that
        // binding is always `Uuid`; a future heterogeneous edge would
        // bind a different node-id type without changing this trait.
        fn _accepts(_e: &dyn Edge<NodeId = Uuid>) {}
        let s = SpawnsEdge::new(Uuid::new_v4(), Uuid::new_v4());
        let b = BlocksEdge::new(Uuid::new_v4(), Uuid::new_v4(), Severity::Critical);
        let r = RelatesEdge::new(Uuid::new_v4(), Uuid::new_v4(), RelatesKind::General);
        _accepts(&s);
        _accepts(&b);
        _accepts(&r);
    }

    #[test]
    fn test_spawns_edge_serializes_without_metadata_field() {
        // SpawnsEdge has no kind-specific fields; the serialised JSON
        // should be exactly the EdgeBase shape — no extra keys, no
        // null placeholders. Pins the goal of dropping the previous
        // `edge_type: null` / `weight: null` noise.
        let e = SpawnsEdge::new(Uuid::nil(), Uuid::from_u128(0x42));
        let json = serde_json::to_value(&e).unwrap();
        let obj = json.as_object().unwrap();
        let keys: Vec<_> = obj.keys().cloned().collect();
        keys.iter()
            .find(|k| k.as_str() == "source")
            .expect("source key");
        keys.iter()
            .find(|k| k.as_str() == "target")
            .expect("target key");
        // No "severity", no "kind", no "edge_type", no "weight"
        for unexpected in ["severity", "kind", "edge_type", "weight", "direction"] {
            assert!(
                !keys.iter().any(|k| k == unexpected),
                "SpawnsEdge should not serialise {unexpected}; got keys {keys:?}"
            );
        }
    }

    #[test]
    fn test_blocks_edge_serialises_severity_inline() {
        // #[serde(flatten)] on the base means BlocksEdge's JSON has
        // source/target/created_at/archived_at at the top level
        // alongside `severity`. No nested `base: {...}` object.
        let e = BlocksEdge::new(Uuid::nil(), Uuid::from_u128(0x42), Severity::High);
        let json = serde_json::to_value(&e).unwrap();
        let obj = json.as_object().unwrap();
        assert_eq!(obj["severity"], "High");
        assert!(obj.contains_key("source"));
        assert!(obj.contains_key("target"));
        assert!(
            !obj.contains_key("base"),
            "flatten should inline EdgeBase, no `base` wrapper key; got {obj:?}"
        );
    }

    #[test]
    fn test_blocks_edge_deserialises_missing_severity_as_default() {
        // Migration tolerance: a blocks edge loaded from a file
        // written before severity existed defaults to Medium.
        let json = serde_json::json!({
            "source": Uuid::nil(),
            "target": Uuid::from_u128(0x42),
            "created_at": "2024-01-01T00:00:00Z",
            "archived_at": null,
        });
        let e: BlocksEdge = serde_json::from_value(json).unwrap();
        assert_eq!(e.severity, Severity::Medium);
    }

    #[test]
    fn test_relates_edge_deserialises_missing_kind_as_default() {
        let json = serde_json::json!({
            "source": Uuid::nil(),
            "target": Uuid::from_u128(0x42),
            "created_at": "2024-01-01T00:00:00Z",
            "archived_at": null,
        });
        let e: RelatesEdge = serde_json::from_value(json).unwrap();
        assert_eq!(e.kind, RelatesKind::General);
    }
}
