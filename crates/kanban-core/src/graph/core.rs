use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::algorithms;
use super::edge::Edge;

/// Generic edge-list container over any `E: Edge`.
///
/// Stores edges as a flat list for efficient serialization. Exposes
/// only kind-agnostic operations: anything that depends on direction
/// or per-kind matching semantics (e.g. "is this an undirected edge
/// between a and b in either order?") lives on the sub-graph type
/// that wraps `EdgeStore`.
///
/// `E: Edge` lets node-level cascade ops (`archive_node`,
/// `unarchive_node`, `remove_node`) and traversal queries
/// (`outgoing`, `incoming`, `adjacency_list`) work without knowing
/// `E`'s concrete metadata. The graph machinery is reusable across
/// any kanban-domain or external kind that satisfies the trait.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeStore<E> {
    edges: Vec<E>,
}

impl<E> Default for EdgeStore<E> {
    fn default() -> Self {
        Self { edges: Vec::new() }
    }
}

impl<E> EdgeStore<E> {
    /// Create an empty edge store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push an edge onto the store, skipping structural invariants.
    ///
    /// Crate-private: the only legitimate callers are
    /// `DagGraph::add_edge_with_metadata` and
    /// `UndirectedGraph::add_edge_with_metadata`, both of which run
    /// the relevant invariants (self-ref, duplicate, cycle) before
    /// pushing. Exposing this further would let an external wrapper
    /// silently bypass those checks — the trait surface (`Graph`,
    /// `Directed`, `Undirected`) is the public entry point.
    pub(crate) fn add_edge(&mut self, edge: E) {
        self.edges.push(edge);
    }

    /// Retain only edges matching `f`. The kind-aware
    /// `remove_directed_edge` / `remove_undirected_edge` helpers
    /// below build on this; sub-graphs that need other matching
    /// semantics can call `retain` directly.
    pub fn retain<F: FnMut(&E) -> bool>(&mut self, f: F) {
        self.edges.retain(f);
    }

    /// Borrow every edge (active + archived).
    pub fn edges(&self) -> &[E] {
        &self.edges
    }

    /// Total edge count (active + archived).
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

impl<E: Edge> EdgeStore<E> {
    /// Remove every edge involving `node` (hard-delete cascade).
    pub fn remove_node(&mut self, node_id: E::NodeId) {
        self.edges.retain(|e| !e.involves(node_id));
    }

    /// Archive every edge involving `node` (soft-delete cascade).
    pub fn archive_node(&mut self, node_id: E::NodeId) {
        for edge in &mut self.edges {
            if edge.involves(node_id) {
                edge.archive();
            }
        }
    }

    /// Unarchive the edges involving `node` whose OTHER endpoint is also live.
    ///
    /// `node` is the node being restored (treated as live). An edge is only
    /// revived when `is_live(other_endpoint)` holds: reviving an edge to a
    /// still-archived neighbor would resurrect a dependency pointing at a hidden
    /// node, violating the born-archived invariant enforced on edge creation.
    pub fn unarchive_node(&mut self, node_id: E::NodeId, is_live: &dyn Fn(E::NodeId) -> bool) {
        for edge in &mut self.edges {
            if edge.involves(node_id) {
                let other = if edge.source() == node_id {
                    edge.target()
                } else {
                    edge.source()
                };
                if is_live(other) {
                    edge.unarchive();
                }
            }
        }
    }

    /// Remove the single **active** edge whose `source == source` and
    /// `target == target` exactly (directed-graph semantics). Archived
    /// edges with the same endpoints are preserved — they're history,
    /// not part of the current view, and a remove of the current edge
    /// must not silently destroy the history record. Returns `true`
    /// iff an active edge was removed.
    pub fn remove_directed_edge(&mut self, source: E::NodeId, target: E::NodeId) -> bool {
        let before = self.edges.len();
        self.edges
            .retain(|e| !(e.is_active() && e.source() == source && e.target() == target));
        self.edges.len() < before
    }

    /// Remove any **active** edge whose endpoints are `{a, b}`
    /// regardless of ordering (undirected-graph semantics). Archived
    /// edges are preserved. Returns `true` iff at least one active
    /// edge was removed.
    pub fn remove_undirected_edge(&mut self, a: E::NodeId, b: E::NodeId) -> bool {
        let before = self.edges.len();
        self.edges.retain(|e| {
            !(e.is_active()
                && ((e.source() == a && e.target() == b) || (e.source() == b && e.target() == a)))
        });
        self.edges.len() < before
    }

    /// Iterate every outgoing edge from `node` (source == node).
    pub fn outgoing(&self, node_id: E::NodeId) -> impl Iterator<Item = &E> {
        self.edges.iter().filter(move |e| e.source() == node_id)
    }

    /// Iterate every incoming edge to `node` (target == node).
    pub fn incoming(&self, node_id: E::NodeId) -> impl Iterator<Item = &E> {
        self.edges.iter().filter(move |e| e.target() == node_id)
    }

    /// Iterate active outgoing edges from `node`.
    pub fn outgoing_active(&self, node_id: E::NodeId) -> impl Iterator<Item = &E> {
        self.edges
            .iter()
            .filter(move |e| e.source() == node_id && e.is_active())
    }

    /// Iterate active incoming edges to `node`.
    pub fn incoming_active(&self, node_id: E::NodeId) -> impl Iterator<Item = &E> {
        self.edges
            .iter()
            .filter(move |e| e.target() == node_id && e.is_active())
    }

    /// Iterate every active edge.
    pub fn active_edges(&self) -> impl Iterator<Item = &E> {
        self.edges.iter().filter(|e| e.is_active())
    }

    /// Active-edge directed adjacency list: `source -> [target]`.
    /// Sub-graphs that need a different view (e.g. undirected
    /// neighbours) build their own from `active_edges`.
    pub fn adjacency_list(&self) -> HashMap<E::NodeId, Vec<E::NodeId>> {
        let mut adj_list: HashMap<E::NodeId, Vec<E::NodeId>> = HashMap::new();
        for edge in self.active_edges() {
            adj_list
                .entry(edge.source())
                .or_default()
                .push(edge.target());
        }
        adj_list
    }

    /// Active-edge count.
    pub fn active_edge_count(&self) -> usize {
        self.edges.iter().filter(|e| e.is_active()).count()
    }

    /// Would adding `source -> target` create a cycle in the active
    /// directed adjacency?
    pub fn would_create_cycle(&self, source: E::NodeId, target: E::NodeId) -> bool {
        let adj_list = self.adjacency_list();
        algorithms::would_create_cycle(&adj_list, source, target)
    }

    /// Does the active directed adjacency contain any cycle?
    pub fn has_cycle(&self) -> bool {
        let adj_list = self.adjacency_list();
        algorithms::has_cycle(&adj_list)
    }

    /// Set of nodes reachable from `start` via the active directed
    /// adjacency.
    pub fn reachable_from(&self, start: E::NodeId) -> std::collections::HashSet<E::NodeId> {
        let adj_list = self.adjacency_list();
        algorithms::reachable_from(&adj_list, start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::edge::EdgeBase;
    use uuid::Uuid;

    fn base(source: Uuid, target: Uuid) -> EdgeBase {
        EdgeBase::new(source, target)
    }

    #[test]
    fn test_new_returns_empty_edge_store() {
        let graph: EdgeStore<EdgeBase> = EdgeStore::new();
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_add_edge_increments_count() {
        let mut graph: EdgeStore<EdgeBase> = EdgeStore::new();
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        graph.add_edge(base(source, target));
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn test_remove_directed_edge_only_matches_exact_orientation() {
        let mut graph: EdgeStore<EdgeBase> = EdgeStore::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        graph.add_edge(base(a, b));
        assert!(!graph.remove_directed_edge(b, a), "wrong orientation");
        assert_eq!(graph.edge_count(), 1);
        assert!(graph.remove_directed_edge(a, b));
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_remove_undirected_edge_matches_either_orientation() {
        let mut graph: EdgeStore<EdgeBase> = EdgeStore::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        graph.add_edge(base(a, b));
        assert!(
            graph.remove_undirected_edge(b, a),
            "symmetric on either ordering"
        );
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_remove_directed_edge_preserves_archived_record() {
        let mut graph: EdgeStore<EdgeBase> = EdgeStore::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        let mut archived = base(a, b);
        archived.archive();
        graph.add_edge(archived);
        graph.add_edge(base(a, b)); // active

        assert!(graph.remove_directed_edge(a, b));
        assert_eq!(
            graph.edge_count(),
            1,
            "archived record must survive the active remove"
        );
        assert_eq!(graph.active_edge_count(), 0);
    }

    #[test]
    fn test_remove_directed_edge_returns_false_when_only_archived_exists() {
        let mut graph: EdgeStore<EdgeBase> = EdgeStore::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let mut archived = base(a, b);
        archived.archive();
        graph.add_edge(archived);

        assert!(
            !graph.remove_directed_edge(a, b),
            "no active edge means remove is a no-op"
        );
        assert_eq!(graph.edge_count(), 1, "archived record untouched");
    }

    #[test]
    fn test_remove_undirected_edge_preserves_archived_record() {
        let mut graph: EdgeStore<EdgeBase> = EdgeStore::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        let mut archived = base(a, b);
        archived.archive();
        graph.add_edge(archived);
        graph.add_edge(base(b, a)); // active in opposite ordering

        assert!(graph.remove_undirected_edge(a, b));
        assert_eq!(graph.edge_count(), 1);
        assert_eq!(graph.active_edge_count(), 0);
    }

    #[test]
    fn test_remove_node_drops_every_incident_edge() {
        let mut graph: EdgeStore<EdgeBase> = EdgeStore::new();
        let node_a = Uuid::new_v4();
        let node_b = Uuid::new_v4();
        let node_c = Uuid::new_v4();

        graph.add_edge(base(node_a, node_b));
        graph.add_edge(base(node_b, node_c));
        graph.add_edge(base(node_c, node_a));

        graph.remove_node(node_b);
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn test_archive_node_hides_incident_edges_from_active_count() {
        let mut graph: EdgeStore<EdgeBase> = EdgeStore::new();
        let node_a = Uuid::new_v4();
        let node_b = Uuid::new_v4();
        let node_c = Uuid::new_v4();

        graph.add_edge(base(node_a, node_b));
        graph.add_edge(base(node_b, node_c));

        assert_eq!(graph.active_edge_count(), 2);

        graph.archive_node(node_b);
        assert_eq!(graph.edge_count(), 2);
        assert_eq!(graph.active_edge_count(), 0);
    }

    #[test]
    fn test_unarchive_node_restores_active_count() {
        let mut graph: EdgeStore<EdgeBase> = EdgeStore::new();
        let node_a = Uuid::new_v4();
        let node_b = Uuid::new_v4();

        graph.add_edge(base(node_a, node_b));
        graph.archive_node(node_a);
        assert_eq!(graph.active_edge_count(), 0);

        graph.unarchive_node(node_a, &|_| true);
        assert_eq!(graph.active_edge_count(), 1);
    }

    #[test]
    fn test_unarchive_node_keeps_edge_to_still_archived_neighbor_tombstoned() {
        let mut graph: EdgeStore<EdgeBase> = EdgeStore::new();
        let node_a = Uuid::new_v4();
        let node_b = Uuid::new_v4();

        graph.add_edge(base(node_a, node_b));
        graph.archive_node(node_b);
        graph.archive_node(node_a);
        assert_eq!(graph.active_edge_count(), 0);

        // Restore only node_a; node_b stays archived. The shared edge must NOT
        // revive — an active edge to a hidden node breaks the born-archived
        // invariant.
        graph.unarchive_node(node_a, &|n| n == node_a);
        assert_eq!(
            graph.active_edge_count(),
            0,
            "edge to still-archived node_b must stay tombstoned"
        );
    }

    #[test]
    fn test_outgoing_and_incoming_partition_by_endpoint_role() {
        let mut graph: EdgeStore<EdgeBase> = EdgeStore::new();
        let node_a = Uuid::new_v4();
        let node_b = Uuid::new_v4();
        let node_c = Uuid::new_v4();

        graph.add_edge(base(node_a, node_b));
        graph.add_edge(base(node_a, node_c));
        graph.add_edge(base(node_c, node_a));

        assert_eq!(graph.outgoing(node_a).count(), 2);
        assert_eq!(graph.incoming(node_a).count(), 1);
        assert_eq!(graph.outgoing(node_b).count(), 0);
        assert_eq!(graph.incoming(node_b).count(), 1);
    }

    #[test]
    fn test_would_create_cycle_detects_directed_path_closing() {
        let mut graph: EdgeStore<EdgeBase> = EdgeStore::new();
        let node_a = Uuid::new_v4();
        let node_b = Uuid::new_v4();
        let node_c = Uuid::new_v4();

        graph.add_edge(base(node_a, node_b));
        graph.add_edge(base(node_b, node_c));

        assert!(graph.would_create_cycle(node_c, node_a));
        assert!(graph.would_create_cycle(node_c, node_b));
    }

    #[test]
    fn test_adjacency_list_counts_active_outgoing_per_node() {
        let mut graph: EdgeStore<EdgeBase> = EdgeStore::new();
        let node_a = Uuid::new_v4();
        let node_b = Uuid::new_v4();
        let node_c = Uuid::new_v4();

        graph.add_edge(base(node_a, node_b));
        graph.add_edge(base(node_b, node_c));

        let adj_list = graph.adjacency_list();
        assert_eq!(adj_list.len(), 2);
        assert_eq!(adj_list.get(&node_a).unwrap().len(), 1);
        assert_eq!(adj_list.get(&node_b).unwrap().len(), 1);
    }
}
