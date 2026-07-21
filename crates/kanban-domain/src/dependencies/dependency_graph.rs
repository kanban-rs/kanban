use kanban_core::{
    Cascadable, DagGraph, Directed, EdgeSet, GraphError, Undirected, UndirectedGraph,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::edges::{BlocksEdge, RelatesEdge, SpawnsEdge};
use crate::error::DependencyError;
use crate::{CardId, KanbanResult};

/// Top-level container for all entity dependency graphs.
///
/// Three discrete sub-graphs, each with its own structural rules
/// and its own concrete edge kind (carrying any per-kind metadata):
///
/// - `spawns: DagGraph<SpawnsEdge>` (no extra metadata today)
/// - `blocks: DagGraph<BlocksEdge>` (carries [`Severity`])
/// - `relates: UndirectedGraph<RelatesEdge>` (carries [`RelatesKind`])
///
/// Cross-cutting operations (`archive_node`, `unarchive_node`,
/// `remove_node`) cascade across all three. Per-edge-type convenience
/// methods delegate to the matching sub-graph and convert
/// [`GraphError`] into the domain-level [`DependencyError`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DependencyGraph {
    #[serde(default)]
    spawns: DagGraph<SpawnsEdge>,
    #[serde(default)]
    blocks: DagGraph<BlocksEdge>,
    #[serde(default)]
    relates: UndirectedGraph<RelatesEdge>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Borrow each component graph as `&mut dyn Cascadable<NodeId = Uuid>`
    /// for node-level cascade operations. Order is `spawns`, `blocks`,
    /// `relates`. A new component sub-graph only needs to be added to
    /// this helper, not to every cascade method.
    ///
    /// The explicit `NodeId = Uuid` binding picks the kanban-domain
    /// identity; the underlying `Cascadable` trait is generic over
    /// `NodeId` so a heterogeneous-entity graph can pick its own
    /// without touching this method.
    fn cascadable_parts_mut(&mut self) -> [&mut dyn Cascadable<NodeId = Uuid>; 3] {
        [&mut self.spawns, &mut self.blocks, &mut self.relates]
    }

    /// Borrow each component graph as `&dyn EdgeSet<NodeId = Uuid>`
    /// for read-only edge-level queries.
    fn edge_sets(&self) -> [&dyn EdgeSet<NodeId = Uuid>; 3] {
        [&self.spawns, &self.blocks, &self.relates]
    }

    // --- Cross-cutting node cascades (Cascadable surface) ---

    pub fn archive_node(&mut self, card: CardId) {
        for sg in self.cascadable_parts_mut() {
            sg.archive_node(card);
        }
    }

    pub fn unarchive_node(&mut self, card: CardId, is_live: &dyn Fn(CardId) -> bool) {
        for sg in self.cascadable_parts_mut() {
            sg.unarchive_node(card, is_live);
        }
    }

    pub fn remove_node(&mut self, card: CardId) {
        for sg in self.cascadable_parts_mut() {
            sg.remove_node(card);
        }
    }

    // --- Cross-cutting edge aggregates (EdgeSet surface) ---

    pub fn len(&self) -> usize {
        self.edge_sets().iter().map(|g| g.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn active_len(&self) -> usize {
        self.edge_sets().iter().map(|g| g.active_len()).sum()
    }

    // --- Parent / child (Spawns) ---

    pub fn set_parent(&mut self, child: CardId, parent: CardId) -> KanbanResult<()> {
        self.spawns
            .add_edge_with_metadata(SpawnsEdge::new(parent, child))
            .map_err(dep_err)
    }

    /// Insert a parent->child Spawns edge already in the archived
    /// state. Used by cascade-undo (`DeleteCard` / `DeleteCardEdges`)
    /// to restore archived incident edges as archived, not active.
    /// `DagGraph::add_edge_with_metadata` skips the duplicate and
    /// cycle checks for archived edges, so this is structurally
    /// equivalent to load-time rehydration of a historical edge.
    pub fn add_archived_spawns(&mut self, parent: CardId, child: CardId) -> KanbanResult<()> {
        use kanban_core::Edge as _;
        let mut edge = SpawnsEdge::new(parent, child);
        edge.archive();
        self.spawns.add_edge_with_metadata(edge).map_err(dep_err)
    }

    pub fn remove_parent(&mut self, child: CardId, parent: CardId) -> KanbanResult<()> {
        // Use the structural `Graph::remove_edge` via the trait so the
        // sub-graph picks the right matching semantics (directed for
        // DAG sub-graphs).
        use kanban_core::Graph as _;
        self.spawns.remove_edge(parent, child).map_err(dep_err)
    }

    pub fn children(&self, parent: CardId) -> Vec<CardId> {
        self.spawns.outgoing(parent)
    }

    pub fn parents(&self, child: CardId) -> Vec<CardId> {
        self.spawns.incoming(child)
    }

    pub fn ancestors(&self, child: CardId) -> Vec<CardId> {
        self.spawns.ancestors(child)
    }

    pub fn descendants(&self, parent: CardId) -> Vec<CardId> {
        self.spawns.descendants(parent)
    }

    pub fn child_count(&self, parent: CardId) -> usize {
        self.spawns.outgoing(parent).len()
    }

    // --- Blocks ---

    /// Add a `blocker -> blocked` edge with default
    /// ([`Severity::Medium`]) severity.
    pub fn set_block(&mut self, blocker: CardId, blocked: CardId) -> KanbanResult<()> {
        self.set_block_with_severity(blocker, blocked, super::Severity::default())
    }

    /// Add a `blocker -> blocked` edge with an explicit severity.
    pub fn set_block_with_severity(
        &mut self,
        blocker: CardId,
        blocked: CardId,
        severity: super::Severity,
    ) -> KanbanResult<()> {
        self.blocks
            .add_edge_with_metadata(BlocksEdge::new(blocker, blocked, severity))
            .map_err(dep_err)
    }

    /// Insert a blocker->blocked Blocks edge already in the archived
    /// state, preserving the supplied `severity`. See
    /// [`add_archived_spawns`] for the cascade-undo rationale.
    pub fn add_archived_blocks(
        &mut self,
        blocker: CardId,
        blocked: CardId,
        severity: super::Severity,
    ) -> KanbanResult<()> {
        use kanban_core::Edge as _;
        let mut edge = BlocksEdge::new(blocker, blocked, severity);
        edge.archive();
        self.blocks.add_edge_with_metadata(edge).map_err(dep_err)
    }

    pub fn unblock(&mut self, blocker: CardId, blocked: CardId) -> KanbanResult<()> {
        use kanban_core::Graph as _;
        self.blocks.remove_edge(blocker, blocked).map_err(dep_err)
    }

    pub fn blocked(&self, card: CardId) -> Vec<CardId> {
        self.blocks.outgoing(card)
    }

    pub fn blockers(&self, card: CardId) -> Vec<CardId> {
        self.blocks.incoming(card)
    }

    pub fn can_start<F>(&self, card: CardId, is_complete: F) -> bool
    where
        F: Fn(CardId) -> bool,
    {
        self.blockers(card).into_iter().all(is_complete)
    }

    // --- Relates ---

    /// Add an undirected `a <-> b` relates edge with default
    /// ([`RelatesKind::General`]) kind.
    pub fn relate(&mut self, a: CardId, b: CardId) -> KanbanResult<()> {
        self.relate_with_kind(a, b, super::RelatesKind::default())
    }

    /// Add an undirected `a <-> b` relates edge with an explicit
    /// sub-kind.
    pub fn relate_with_kind(
        &mut self,
        a: CardId,
        b: CardId,
        kind: super::RelatesKind,
    ) -> KanbanResult<()> {
        self.relates
            .add_edge_with_metadata(RelatesEdge::new(a, b, kind))
            .map_err(dep_err)
    }

    /// Insert an undirected RelatesTo edge already in the archived
    /// state, preserving the supplied `kind`. See
    /// [`add_archived_spawns`] for the cascade-undo rationale.
    pub fn add_archived_relates(
        &mut self,
        a: CardId,
        b: CardId,
        kind: super::RelatesKind,
    ) -> KanbanResult<()> {
        use kanban_core::Edge as _;
        let mut edge = RelatesEdge::new(a, b, kind);
        edge.archive();
        self.relates.add_edge_with_metadata(edge).map_err(dep_err)
    }

    pub fn dissociate(&mut self, a: CardId, b: CardId) -> KanbanResult<()> {
        use kanban_core::Graph as _;
        self.relates.remove_edge(a, b).map_err(dep_err)
    }

    pub fn related(&self, card: CardId) -> Vec<CardId> {
        self.relates.neighbors(card)
    }

    /// True iff an **active** edge between `a` and `b` exists in any
    /// sub-graph. Use this to ask "is there a current dependency
    /// here?". Archived edges are not counted; use
    /// `contains_archived` to consult both.
    pub fn contains(&self, a: Uuid, b: Uuid) -> bool {
        self.edge_sets().iter().any(|g| g.contains(a, b))
    }

    /// True iff any edge between `a` and `b` exists in any sub-graph,
    /// including archived edges. Use when reasoning about edge
    /// history.
    pub fn contains_archived(&self, a: Uuid, b: Uuid) -> bool {
        self.edge_sets().iter().any(|g| g.contains_archived(a, b))
    }

    // --- Persistence helpers ---

    /// Per-kind raw edge accessors. Persistence backends and test
    /// fixtures use these to round-trip metadata-bearing edges. The
    /// previous kind-discriminated `edge_bases_of(kind)` was dropped:
    /// callers either know which kind they want (per-kind accessor)
    /// or want them all (iterate all three).
    pub fn spawns_edges(&self) -> &[SpawnsEdge] {
        self.spawns.edges()
    }
    pub fn blocks_edges(&self) -> &[BlocksEdge] {
        self.blocks.edges()
    }
    pub fn relates_edges(&self) -> &[RelatesEdge] {
        self.relates.edges()
    }

    /// Construct a graph from per-kind edge vectors with structural
    /// validation. Each sub-graph independently checks its
    /// self-reference / cycle / duplicate invariants on the way in; a
    /// corrupted load fails up front instead of silently rehydrating
    /// an invariant-violating graph.
    ///
    /// On failure the returned error names the offending edge kind
    /// and endpoints so a user inspecting a load failure has enough
    /// information to find the bad row in the source file (the
    /// anonymous `DependencyError` variant alone would only say
    /// "cycle detected" with no clue which kind or which edge).
    pub fn from_validated_per_kind_edges(
        spawns: Vec<SpawnsEdge>,
        blocks: Vec<BlocksEdge>,
        relates: Vec<RelatesEdge>,
    ) -> KanbanResult<Self> {
        use kanban_core::Edge as _;
        let mut graph = Self::new();
        for edge in spawns {
            let (s, t) = (edge.source(), edge.target());
            graph
                .spawns
                .add_edge_with_metadata(edge)
                .map_err(|e| load_err_with_context(e, "spawns", s, t))?;
        }
        for edge in blocks {
            let (s, t) = (edge.source(), edge.target());
            graph
                .blocks
                .add_edge_with_metadata(edge)
                .map_err(|e| load_err_with_context(e, "blocks", s, t))?;
        }
        for edge in relates {
            let (s, t) = (edge.source(), edge.target());
            graph
                .relates
                .add_edge_with_metadata(edge)
                .map_err(|e| load_err_with_context(e, "relates", s, t))?;
        }
        Ok(graph)
    }
}

/// Wrap a structural `GraphError` from a load path with the kind tag
/// and offending endpoints, so a corrupt-file diagnostic identifies
/// the bad row instead of just naming the invariant.
fn load_err_with_context(
    e: GraphError,
    kind: &'static str,
    source: Uuid,
    target: Uuid,
) -> crate::KanbanError {
    crate::KanbanError::validation(format!(
        "load failed on {kind} edge {source} -> {target}: {e}"
    ))
}

fn dep_err(e: GraphError) -> crate::KanbanError {
    let d: DependencyError = e.into();
    d.into()
}

impl From<GraphError> for DependencyError {
    fn from(e: GraphError) -> Self {
        match e {
            GraphError::Cycle => DependencyError::CycleDetected,
            GraphError::SelfReference => DependencyError::SelfReference,
            GraphError::EdgeNotFound => DependencyError::EdgeNotFound,
            GraphError::Duplicate => DependencyError::DuplicateEdge,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn ids() -> (Uuid, Uuid, Uuid) {
        (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4())
    }

    #[test]
    fn test_new_returns_empty_graph() {
        let graph = DependencyGraph::new();
        assert_eq!(graph.len(), 0);
    }

    #[test]
    fn test_default_graph_round_trips_through_serde() {
        let graph = DependencyGraph::new();
        let json = serde_json::to_string(&graph).unwrap();
        let deserialized: DependencyGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), 0);
    }

    #[test]
    fn test_dependency_graph_serializes_spawns_bucket_key() {
        // The on-disk JSON bucket name must match the domain
        // vocabulary (SpawnsEdge, spawns_edges(), SQLite
        // spawns_edges table). The historical Rust field name
        // `parent_child` leaked into the wire format; this pins
        // the rename to `spawns`.
        let graph = DependencyGraph::new();
        let json = serde_json::to_value(&graph).unwrap();
        let obj = json.as_object().expect("graph serialises to a JSON object");
        assert!(
            obj.contains_key("spawns"),
            "DependencyGraph must serialise the spawns bucket under key `spawns`; got keys {:?}",
            obj.keys().collect::<Vec<_>>()
        );
        assert!(
            !obj.contains_key("parent_child"),
            "legacy key `parent_child` must be gone from the wire format; got keys {:?}",
            obj.keys().collect::<Vec<_>>()
        );
    }

    // --- Parent/child (Spawns) ---

    // --- from_validated_per_kind_edges context ---
    //
    // A corrupt load (cycle, self-ref, duplicate) must surface enough
    // information for a user to find the bad row in the source file.
    // The bare DependencyError variants don't carry context; the load
    // path wraps them with the kind tag and the offending endpoints.

    #[test]
    fn test_load_with_blocks_cycle_names_kind_and_endpoints() {
        let (a, b, c) = ids();
        // Construct a cycle by going through the validated path with
        // a chain a->b, b->c, c->a in the blocks vector.
        let err = DependencyGraph::from_validated_per_kind_edges(
            Vec::new(),
            vec![
                super::super::BlocksEdge::new(a, b, super::super::Severity::Medium),
                super::super::BlocksEdge::new(b, c, super::super::Severity::Medium),
                super::super::BlocksEdge::new(c, a, super::super::Severity::Medium),
            ],
            Vec::new(),
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("blocks"),
            "load error must name the kind; got: {msg}"
        );
        assert!(
            msg.contains(&c.to_string()) && msg.contains(&a.to_string()),
            "load error must name the offending endpoints; got: {msg}"
        );
        assert!(
            msg.to_lowercase().contains("cycle"),
            "load error must name the invariant; got: {msg}"
        );
    }

    #[test]
    fn test_load_with_relates_self_reference_names_kind_and_endpoint() {
        let a = Uuid::new_v4();
        let err = DependencyGraph::from_validated_per_kind_edges(
            Vec::new(),
            Vec::new(),
            vec![super::super::RelatesEdge::new(
                a,
                a,
                super::super::RelatesKind::General,
            )],
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("relates"), "kind name in message: {msg}");
        assert!(msg.contains(&a.to_string()), "endpoint in message: {msg}");
    }

    #[test]
    fn test_set_parent_creates_parent_child_edge() {
        let (parent, child, _) = ids();
        let mut g = DependencyGraph::new();
        g.set_parent(child, parent).unwrap();
        assert_eq!(g.children(parent), vec![child]);
        assert_eq!(g.parents(child), vec![parent]);
    }

    #[test]
    fn test_set_parent_self_reference_returns_error() {
        let (a, _, _) = ids();
        let mut g = DependencyGraph::new();
        assert!(g.set_parent(a, a).unwrap_err().is_self_reference());
    }

    #[test]
    fn test_set_parent_cycle_returns_error() {
        let (a, b, c) = ids();
        let mut g = DependencyGraph::new();
        g.set_parent(b, a).unwrap();
        g.set_parent(c, b).unwrap();
        assert!(g.set_parent(a, c).unwrap_err().is_cycle_detected());
    }

    #[test]
    fn test_remove_parent_existing_succeeds() {
        let (parent, child, _) = ids();
        let mut g = DependencyGraph::new();
        g.set_parent(child, parent).unwrap();
        g.remove_parent(child, parent).unwrap();
        assert!(g.children(parent).is_empty());
    }

    #[test]
    fn test_set_parent_duplicate_returns_duplicate_edge_error() {
        let (parent, child, _) = ids();
        let mut g = DependencyGraph::new();
        g.set_parent(child, parent).unwrap();
        assert!(g.set_parent(child, parent).unwrap_err().is_duplicate_edge());
        assert_eq!(g.children(parent), vec![child], "no duplicate stored");
    }

    #[test]
    fn test_remove_parent_missing_returns_edge_not_found() {
        let (parent, child, _) = ids();
        let mut g = DependencyGraph::new();
        assert!(g
            .remove_parent(child, parent)
            .unwrap_err()
            .is_edge_not_found());
    }

    #[test]
    fn test_ancestors_returns_transitive_parents() {
        let (gp, p, c) = ids();
        let mut g = DependencyGraph::new();
        g.set_parent(p, gp).unwrap();
        g.set_parent(c, p).unwrap();
        let mut anc = g.ancestors(c);
        anc.sort();
        let mut expected = vec![gp, p];
        expected.sort();
        assert_eq!(anc, expected);
    }

    #[test]
    fn test_descendants_returns_transitive_children() {
        let (parent, child, grandchild) = ids();
        let mut g = DependencyGraph::new();
        g.set_parent(child, parent).unwrap();
        g.set_parent(grandchild, child).unwrap();
        let mut desc = g.descendants(parent);
        desc.sort();
        let mut expected = vec![child, grandchild];
        expected.sort();
        assert_eq!(desc, expected);
    }

    #[test]
    fn test_child_count_matches_children_len() {
        let (parent, a, b) = ids();
        let mut g = DependencyGraph::new();
        assert_eq!(g.child_count(parent), 0);
        g.set_parent(a, parent).unwrap();
        g.set_parent(b, parent).unwrap();
        assert_eq!(g.child_count(parent), 2);
    }

    // --- Blocks ---

    #[test]
    fn test_add_blocks_creates_directed_edge() {
        let (a, b, _) = ids();
        let mut g = DependencyGraph::new();
        g.set_block(a, b).unwrap();
        assert_eq!(g.blocked(a), vec![b]);
        assert_eq!(g.blockers(b), vec![a]);
    }

    #[test]
    fn test_set_block_with_severity_preserves_metadata() {
        let (a, b, _) = ids();
        let mut g = DependencyGraph::new();
        g.set_block_with_severity(a, b, super::super::Severity::Critical)
            .unwrap();
        assert_eq!(
            g.blocks_edges()[0].severity,
            super::super::Severity::Critical
        );
    }

    #[test]
    fn test_set_block_default_is_medium_severity() {
        let (a, b, _) = ids();
        let mut g = DependencyGraph::new();
        g.set_block(a, b).unwrap();
        assert_eq!(g.blocks_edges()[0].severity, super::super::Severity::Medium);
    }

    #[test]
    fn test_add_blocks_cycle_returns_error() {
        let (a, b, c) = ids();
        let mut g = DependencyGraph::new();
        g.set_block(a, b).unwrap();
        g.set_block(b, c).unwrap();
        assert!(g.set_block(c, a).unwrap_err().is_cycle_detected());
    }

    #[test]
    fn test_set_block_duplicate_returns_duplicate_edge_error() {
        let (a, b, _) = ids();
        let mut g = DependencyGraph::new();
        g.set_block(a, b).unwrap();
        assert!(g.set_block(a, b).unwrap_err().is_duplicate_edge());
    }

    #[test]
    fn test_relate_duplicate_in_either_orientation_returns_duplicate_edge_error() {
        let (a, b, _) = ids();
        let mut g = DependencyGraph::new();
        g.relate(a, b).unwrap();
        assert!(g.relate(a, b).unwrap_err().is_duplicate_edge());
        assert!(g.relate(b, a).unwrap_err().is_duplicate_edge());
    }

    #[test]
    fn test_add_blocks_self_reference_returns_error() {
        let (a, _, _) = ids();
        let mut g = DependencyGraph::new();
        assert!(g.set_block(a, a).unwrap_err().is_self_reference());
    }

    #[test]
    fn test_can_start_returns_true_when_all_blockers_complete() {
        let (a, b, c) = ids();
        let mut g = DependencyGraph::new();
        g.set_block(a, c).unwrap();
        g.set_block(b, c).unwrap();
        assert!(!g.can_start(c, |id| id == a));
        assert!(g.can_start(c, |_| true));
    }

    // --- Relates ---

    #[test]
    fn test_add_relates_to_creates_bidirectional_edge() {
        let (a, b, _) = ids();
        let mut g = DependencyGraph::new();
        g.relate(a, b).unwrap();
        assert_eq!(g.related(a), vec![b]);
        assert_eq!(g.related(b), vec![a]);
    }

    #[test]
    fn test_relate_with_kind_preserves_metadata() {
        let (a, b, _) = ids();
        let mut g = DependencyGraph::new();
        g.relate_with_kind(a, b, super::super::RelatesKind::Duplicates)
            .unwrap();
        assert_eq!(
            g.relates_edges()[0].kind,
            super::super::RelatesKind::Duplicates
        );
    }

    #[test]
    fn test_relate_default_is_general_kind() {
        let (a, b, _) = ids();
        let mut g = DependencyGraph::new();
        g.relate(a, b).unwrap();
        assert_eq!(
            g.relates_edges()[0].kind,
            super::super::RelatesKind::General
        );
    }

    #[test]
    fn test_add_relates_to_self_reference_returns_error() {
        let (a, _, _) = ids();
        let mut g = DependencyGraph::new();
        assert!(g.relate(a, a).unwrap_err().is_self_reference());
    }

    #[test]
    fn test_add_relates_permits_cycle() {
        let (a, b, c) = ids();
        let mut g = DependencyGraph::new();
        g.relate(a, b).unwrap();
        g.relate(b, c).unwrap();
        assert!(g.relate(c, a).is_ok());
    }

    // --- Cross-cutting cascades ---

    #[test]
    fn test_archive_node_hides_edges_in_all_subgraphs() {
        let (a, b, c) = ids();
        let mut g = DependencyGraph::new();
        g.set_parent(b, a).unwrap();
        g.set_block(a, c).unwrap();
        g.relate(a, c).unwrap();
        g.archive_node(a);
        assert!(g.children(a).is_empty());
        assert!(g.blocked(a).is_empty());
        assert!(g.related(a).is_empty());
        assert_eq!(g.len(), 3);
        assert_eq!(g.active_len(), 0);
    }

    #[test]
    fn test_unarchive_node_keeps_edge_to_still_archived_neighbor_tombstoned() {
        let (a, b, _) = ids();
        let mut g = DependencyGraph::new();
        g.set_block(a, b).unwrap();
        g.archive_node(b);
        g.archive_node(a);
        // Restore a; b remains archived, so the shared edge must not revive.
        g.unarchive_node(a, &|id| id == a);
        assert!(
            !g.contains(a, b),
            "edge to still-archived b must stay tombstoned after restoring a"
        );
        assert!(g.contains_archived(a, b));
    }

    #[test]
    fn test_unarchive_node_revives_edge_when_neighbor_is_live() {
        let (a, b, _) = ids();
        let mut g = DependencyGraph::new();
        g.set_block(a, b).unwrap();
        g.archive_node(a);
        // b was always live; restoring a brings the edge back.
        g.unarchive_node(a, &|_| true);
        assert!(
            g.contains(a, b),
            "edge revives when both endpoints are live"
        );
    }

    #[test]
    fn test_remove_node_removes_edges_in_all_subgraphs() {
        let (a, b, c) = ids();
        let mut g = DependencyGraph::new();
        g.set_parent(b, a).unwrap();
        g.set_block(a, c).unwrap();
        g.relate(a, c).unwrap();
        g.remove_node(a);
        assert_eq!(g.len(), 0);
    }

    // --- Active-only contains vs contains_archived ---

    #[test]
    fn test_contains_returns_false_for_archived_only_edge() {
        let (a, b, _) = ids();
        let mut g = DependencyGraph::new();
        g.set_block(a, b).unwrap();
        g.archive_node(a);
        assert!(!g.contains(a, b), "active contains() must skip archived");
        assert!(
            g.contains_archived(a, b),
            "contains_archived() picks up the archived edge"
        );
    }

    #[test]
    fn test_contains_returns_true_for_active_edge() {
        let (a, b, _) = ids();
        let mut g = DependencyGraph::new();
        g.set_block(a, b).unwrap();
        assert!(g.contains(a, b));
        assert!(g.contains_archived(a, b));
    }

    #[test]
    fn test_parent_child_and_blocks_are_independent() {
        let (a, b, c) = ids();
        let mut g = DependencyGraph::new();
        g.set_parent(b, a).unwrap();
        g.set_block(b, c).unwrap();
        assert_eq!(g.children(a), vec![b]);
        assert_eq!(g.blocked(b), vec![c]);
    }
}
