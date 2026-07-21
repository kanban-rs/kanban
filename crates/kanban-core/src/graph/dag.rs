use serde::{Deserialize, Deserializer, Serialize};

use super::core::EdgeStore;
use super::edge::Edge;
use super::error::GraphError;
use super::traits::{Cascadable, Directed, EdgeSet, Graph};

/// Directed acyclic graph generic over any `E: Edge`.
///
/// Rejects self-references and any edge whose insertion would
/// create a cycle in the active-edge subgraph. Wraps an
/// [`EdgeStore<E>`] to inherit archive / unarchive semantics for
/// soft-delete cascades.
///
/// `Deserialize` runs the DAG invariants against the active subset
/// of loaded edges; a corrupted file with a cycle or self-reference
/// fails to load up front rather than silently rehydrating into an
/// invariant-violating state.
///
/// The graph machinery is fully generic: external code can
/// instantiate `DagGraph<MyEdge>` for any `MyEdge: Edge` without
/// needing to modify this crate. The kanban-domain `DependencyGraph`
/// uses three concrete instantiations
/// (`DagGraph<SpawnsEdge>` / `DagGraph<BlocksEdge>` / etc.) but the
/// machinery itself is open for extension.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DagGraph<E> {
    #[serde(flatten)]
    store: EdgeStore<E>,
}

impl<E> Default for DagGraph<E> {
    fn default() -> Self {
        Self {
            store: EdgeStore::new(),
        }
    }
}

impl<'de, E> Deserialize<'de> for DagGraph<E>
where
    E: Edge + Clone + Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let store: EdgeStore<E> = EdgeStore::deserialize(deserializer)?;
        let mut graph: DagGraph<E> = DagGraph::default();
        for edge in store.edges() {
            graph
                .add_edge_with_metadata(edge.clone())
                .map_err(serde::de::Error::custom)?;
        }
        Ok(graph)
    }
}

impl<E: Edge> DagGraph<E> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Borrow the raw underlying edge list (active + archived).
    /// Persistence layers use this to serialise; size and membership
    /// queries go through the [`EdgeSet`] trait surface.
    pub fn edges(&self) -> &[E] {
        self.store.edges()
    }

    /// Push an edge while preserving caller-supplied metadata.
    ///
    /// Runs the same self-reference, duplicate, and cycle invariants
    /// as the trait's [`Graph::add_edge`]:
    /// - self-references are rejected always;
    /// - active duplicates (an existing active `source -> target`)
    ///   are rejected always; the duplicate check ignores archived
    ///   edges so re-adding after archive succeeds;
    /// - cycles are rejected when the edge is active, ignored
    ///   when it is archived (archived edges are not part of the
    ///   active DAG and don't constrain new mutations).
    ///
    /// Load paths use this to rehydrate stored edges and surface
    /// corrupt-DAG state as a hard load failure.
    pub fn add_edge_with_metadata(&mut self, edge: E) -> Result<(), GraphError> {
        if edge.source() == edge.target() {
            return Err(GraphError::SelfReference);
        }
        if edge.is_active() {
            if self
                .store
                .outgoing_active(edge.source())
                .any(|e| e.target() == edge.target())
            {
                return Err(GraphError::Duplicate);
            }
            let adj = self.store.adjacency_list();
            if super::algorithms::would_create_cycle(&adj, edge.source(), edge.target()) {
                return Err(GraphError::Cycle);
            }
        }
        self.store.add_edge(edge);
        Ok(())
    }

    /// Transitive successors of `node` (descendants).
    pub fn descendants(&self, node: E::NodeId) -> Vec<E::NodeId> {
        let mut out = Vec::new();
        let mut stack = vec![node];
        let mut seen = std::collections::HashSet::new();
        seen.insert(node);
        while let Some(n) = stack.pop() {
            for next in self.outgoing(n) {
                if seen.insert(next) {
                    out.push(next);
                    stack.push(next);
                }
            }
        }
        out
    }

    /// Transitive predecessors of `node` (ancestors).
    pub fn ancestors(&self, node: E::NodeId) -> Vec<E::NodeId> {
        let mut out = Vec::new();
        let mut stack = vec![node];
        let mut seen = std::collections::HashSet::new();
        seen.insert(node);
        while let Some(n) = stack.pop() {
            for prev in self.incoming(n) {
                if seen.insert(prev) {
                    out.push(prev);
                    stack.push(prev);
                }
            }
        }
        out
    }
}

impl<E: Edge> Cascadable for DagGraph<E> {
    fn archive_node(&mut self, node: E::NodeId) {
        self.store.archive_node(node);
    }
    fn unarchive_node(&mut self, node: E::NodeId, is_live: &dyn Fn(E::NodeId) -> bool) {
        self.store.unarchive_node(node, is_live);
    }
    fn remove_node(&mut self, node: E::NodeId) {
        self.store.remove_node(node);
    }
}

impl<E: Edge> EdgeSet for DagGraph<E> {
    fn len(&self) -> usize {
        self.store.edge_count()
    }
    fn active_len(&self) -> usize {
        self.store.active_edge_count()
    }
    /// Directed active-only membership: source-to-target ordering
    /// matters. Aligned with `Graph::contains_edge`.
    fn contains(&self, a: E::NodeId, b: E::NodeId) -> bool {
        self.store.outgoing_active(a).any(|e| e.target() == b)
    }
    /// Directed any-state membership including archived edges.
    fn contains_archived(&self, a: E::NodeId, b: E::NodeId) -> bool {
        self.store
            .edges()
            .iter()
            .any(|e| e.source() == a && e.target() == b)
    }
}

impl<E: Edge> Graph for DagGraph<E> {
    type NodeId = E::NodeId;

    fn add_edge(&mut self, from: E::NodeId, to: E::NodeId) -> Result<(), GraphError> {
        // Synthesise an E with per-kind default metadata via the
        // trait constructor. Callers that need to set non-default
        // metadata construct the concrete struct (e.g.
        // `BlocksEdge::new(..., Severity::High)`) and push it via
        // `add_edge_with_metadata`.
        self.add_edge_with_metadata(E::from_endpoints(from, to))
    }

    fn remove_edge(&mut self, from: E::NodeId, to: E::NodeId) -> Result<(), GraphError> {
        if self.store.remove_directed_edge(from, to) {
            Ok(())
        } else {
            Err(GraphError::EdgeNotFound)
        }
    }

    fn contains_edge(&self, from: E::NodeId, to: E::NodeId) -> bool {
        self.store.outgoing_active(from).any(|e| e.target() == to)
    }
}

impl<E: Edge> Directed for DagGraph<E> {
    fn outgoing(&self, node: E::NodeId) -> Vec<E::NodeId> {
        self.store
            .outgoing_active(node)
            .map(|e| e.target())
            .collect()
    }

    fn incoming(&self, node: E::NodeId) -> Vec<E::NodeId> {
        self.store
            .incoming_active(node)
            .map(|e| e.source())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::edge::EdgeBase;
    use uuid::Uuid;

    fn ids() -> (Uuid, Uuid, Uuid) {
        (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4())
    }

    fn add(g: &mut DagGraph<EdgeBase>, a: Uuid, b: Uuid) -> Result<(), GraphError> {
        g.add_edge_with_metadata(EdgeBase::new(a, b))
    }

    #[test]
    fn test_new_dag_is_empty() {
        let g: DagGraph<EdgeBase> = DagGraph::new();
        assert_eq!(g.len(), 0);
        assert_eq!(g.active_len(), 0);
    }

    #[test]
    fn test_add_edge_with_metadata_creates_outgoing_and_incoming() {
        let (a, b, _) = ids();
        let mut g: DagGraph<EdgeBase> = DagGraph::new();
        add(&mut g, a, b).unwrap();
        assert_eq!(g.outgoing(a), vec![b]);
        assert_eq!(g.incoming(b), vec![a]);
    }

    #[test]
    fn test_add_edge_with_metadata_self_reference_returns_error() {
        let (a, _, _) = ids();
        let mut g: DagGraph<EdgeBase> = DagGraph::new();
        assert_eq!(add(&mut g, a, a), Err(GraphError::SelfReference));
    }

    #[test]
    fn test_add_edge_with_metadata_creating_cycle_returns_error() {
        let (a, b, c) = ids();
        let mut g: DagGraph<EdgeBase> = DagGraph::new();
        add(&mut g, a, b).unwrap();
        add(&mut g, b, c).unwrap();
        assert_eq!(add(&mut g, c, a), Err(GraphError::Cycle));
    }

    #[test]
    fn test_remove_edge_existing_succeeds() {
        let (a, b, _) = ids();
        let mut g: DagGraph<EdgeBase> = DagGraph::new();
        add(&mut g, a, b).unwrap();
        assert_eq!(g.remove_edge(a, b), Ok(()));
        assert!(g.outgoing(a).is_empty());
    }

    #[test]
    fn test_remove_edge_missing_returns_edge_not_found() {
        let (a, b, _) = ids();
        let mut g: DagGraph<EdgeBase> = DagGraph::new();
        assert_eq!(g.remove_edge(a, b), Err(GraphError::EdgeNotFound));
    }

    #[test]
    fn test_contains_edge_distinguishes_direction() {
        let (a, b, _) = ids();
        let mut g: DagGraph<EdgeBase> = DagGraph::new();
        add(&mut g, a, b).unwrap();
        assert!(g.contains_edge(a, b));
        assert!(!g.contains_edge(b, a));
    }

    #[test]
    fn test_archive_node_removes_from_active_view() {
        let (a, b, _) = ids();
        let mut g: DagGraph<EdgeBase> = DagGraph::new();
        add(&mut g, a, b).unwrap();
        g.archive_node(b);
        assert!(g.outgoing(a).is_empty());
        assert_eq!(g.len(), 1);
        assert_eq!(g.active_len(), 0);
    }

    #[test]
    fn test_unarchive_node_restores_active_view() {
        let (a, b, _) = ids();
        let mut g: DagGraph<EdgeBase> = DagGraph::new();
        add(&mut g, a, b).unwrap();
        g.archive_node(b);
        g.unarchive_node(b, &|_| true);
        assert_eq!(g.outgoing(a), vec![b]);
    }

    #[test]
    fn test_remove_node_deletes_all_involved_edges() {
        let (a, b, c) = ids();
        let mut g: DagGraph<EdgeBase> = DagGraph::new();
        add(&mut g, a, b).unwrap();
        add(&mut g, b, c).unwrap();
        g.remove_node(b);
        assert_eq!(g.len(), 0);
    }

    #[test]
    fn test_descendants_returns_transitive_successors() {
        let (a, b, c) = ids();
        let d = Uuid::new_v4();
        let mut g: DagGraph<EdgeBase> = DagGraph::new();
        add(&mut g, a, b).unwrap();
        add(&mut g, b, c).unwrap();
        add(&mut g, c, d).unwrap();
        let mut got = g.descendants(a);
        got.sort();
        let mut expected = vec![b, c, d];
        expected.sort();
        assert_eq!(got, expected);
    }

    #[test]
    fn test_ancestors_returns_transitive_predecessors() {
        let (a, b, c) = ids();
        let mut g: DagGraph<EdgeBase> = DagGraph::new();
        add(&mut g, a, b).unwrap();
        add(&mut g, b, c).unwrap();
        let mut got = g.ancestors(c);
        got.sort();
        let mut expected = vec![a, b];
        expected.sort();
        assert_eq!(got, expected);
    }

    #[test]
    fn test_add_active_duplicate_returns_duplicate_error() {
        let (a, b, _) = ids();
        let mut g: DagGraph<EdgeBase> = DagGraph::new();
        add(&mut g, a, b).unwrap();
        assert_eq!(
            add(&mut g, a, b),
            Err(GraphError::Duplicate),
            "second active a->b must be rejected as duplicate"
        );
        assert_eq!(
            g.outgoing(a),
            vec![b],
            "rejected duplicate must not appear in adjacency"
        );
    }

    #[test]
    fn test_add_reverse_orientation_is_not_a_duplicate() {
        let (a, b, _) = ids();
        let mut g: DagGraph<EdgeBase> = DagGraph::new();
        add(&mut g, a, b).unwrap();
        // a->b exists; b->a is a different directed edge — but the DAG
        // also rejects it as a cycle (a->b->a). Pinning the precedence:
        // cycle wins over duplicate because cycle covers the structural
        // invariant.
        assert_eq!(
            add(&mut g, b, a),
            Err(GraphError::Cycle),
            "reverse orientation is a cycle in a DAG, not a duplicate"
        );
    }

    #[test]
    fn test_re_add_after_archive_succeeds_and_keeps_both_records() {
        let (a, b, _) = ids();
        let mut g: DagGraph<EdgeBase> = DagGraph::new();
        add(&mut g, a, b).unwrap();
        g.archive_node(a);
        // Archived edge no longer counts toward the duplicate check.
        add(&mut g, a, b).unwrap();
        assert_eq!(g.active_len(), 1, "one fresh active edge");
        assert_eq!(g.len(), 2, "archive record is preserved alongside");
    }

    #[test]
    fn test_archived_edge_is_ignored_for_cycle_check() {
        let (a, b, _) = ids();
        let mut g: DagGraph<EdgeBase> = DagGraph::new();
        add(&mut g, a, b).unwrap();
        g.archive_node(a);
        assert_eq!(add(&mut g, b, a), Ok(()));
    }

    #[test]
    fn test_deserialize_rejects_cycle_in_active_edges() {
        let (a, b, c) = ids();
        let now = chrono::Utc::now();
        let json = serde_json::json!({
            "edges": [
                {"source": a, "target": b, "created_at": now, "archived_at": null},
                {"source": b, "target": c, "created_at": now, "archived_at": null},
                {"source": c, "target": a, "created_at": now, "archived_at": null},
            ]
        });
        let result: Result<DagGraph<EdgeBase>, _> = serde_json::from_value(json);
        let err = result.expect_err("a 3-cycle must fail to deserialize");
        assert!(
            err.to_string().to_lowercase().contains("cycle"),
            "deserialize error should name the cycle invariant: {err}"
        );
    }

    #[test]
    fn test_deserialize_rejects_self_reference() {
        let a = Uuid::new_v4();
        let now = chrono::Utc::now();
        let json = serde_json::json!({
            "edges": [
                {"source": a, "target": a, "created_at": now, "archived_at": null},
            ]
        });
        let result: Result<DagGraph<EdgeBase>, _> = serde_json::from_value(json);
        let err = result.expect_err("self-reference must fail to deserialize");
        assert!(
            err.to_string().to_lowercase().contains("self"),
            "deserialize error should name the self-reference invariant: {err}"
        );
    }

    /// The graph machinery is parameterised over the `Edge::NodeId`
    /// associated type, not pinned to `Uuid`. This test pins the
    /// abstraction unlock by instantiating a `DagGraph` over an edge
    /// whose endpoints are `u32` rather than `Uuid` and exercising
    /// cycle / reachability over it. If the unlock ever regresses
    /// (e.g. someone re-hardcodes `Uuid` in algorithms.rs or
    /// EdgeStore), this test fails to compile.
    #[test]
    fn test_dag_graph_works_with_non_uuid_node_id() {
        use crate::graph::edge::EdgeBase;
        let mut g: DagGraph<EdgeBase<u32>> = DagGraph::new();
        g.add_edge_with_metadata(EdgeBase::new(1u32, 2u32)).unwrap();
        g.add_edge_with_metadata(EdgeBase::new(2u32, 3u32)).unwrap();
        assert_eq!(
            g.add_edge_with_metadata(EdgeBase::new(3u32, 1u32)),
            Err(GraphError::Cycle),
            "cycle detection works over u32 node ids"
        );
        assert_eq!(g.outgoing(1), vec![2u32]);
        assert_eq!(g.incoming(3), vec![2u32]);
        let mut desc = g.descendants(1);
        desc.sort();
        assert_eq!(desc, vec![2u32, 3u32]);
    }

    #[test]
    fn test_deserialize_accepts_archived_edge_completing_cycle() {
        let (a, b, c) = ids();
        let now = chrono::Utc::now();
        let json = serde_json::json!({
            "edges": [
                {"source": a, "target": b, "created_at": now, "archived_at": null},
                {"source": b, "target": c, "created_at": now, "archived_at": null},
                {"source": c, "target": a, "created_at": now, "archived_at": now},
            ]
        });
        let graph: DagGraph<EdgeBase> =
            serde_json::from_value(json).expect("cycle through archived edge must be loadable");
        assert_eq!(graph.len(), 3);
        assert_eq!(graph.active_len(), 2);
    }
}
