use uuid::Uuid;

use super::error::GraphError;

// Note on the `Uuid` import: only the object-safety smoke tests use
// it as a concrete binding for the `NodeId` associated type. The
// traits themselves are now fully generic over `NodeId`.

/// Trait for entities that can participate in a graph
///
/// Implemented by domain entities like Card, Sprint, Board, etc.
/// Provides a unique identifier for graph node operations.
pub trait GraphNode {
    /// Get the unique identifier for this node
    fn node_id(&self) -> Uuid;
}

/// Minimal contract every graph data structure satisfies.
///
/// Direction-agnostic: only methods that make sense for both directed
/// and undirected graphs live here. Directional access (`outgoing` /
/// `incoming`) lives on [`Directed`]; undirected neighbour access lives
/// on [`Undirected`]. A type that needs both must pick the right
/// subtype — there is no way to ask an [`Undirected`] graph for
/// directed semantics and silently get a wrong answer.
///
/// Object-safe with an explicit `NodeId` binding, e.g.
/// `&dyn Graph<NodeId = Uuid>`.
pub trait Graph {
    type NodeId: Copy + Eq + std::hash::Hash;

    /// Add an edge `from -> to`. Returns [`GraphError::Cycle`] /
    /// [`GraphError::SelfReference`] when the implementation rejects.
    fn add_edge(&mut self, from: Self::NodeId, to: Self::NodeId) -> Result<(), GraphError>;

    /// Remove the edge `from -> to`. Returns [`GraphError::EdgeNotFound`]
    /// if no such edge exists.
    fn remove_edge(&mut self, from: Self::NodeId, to: Self::NodeId) -> Result<(), GraphError>;

    /// True iff an edge `from -> to` is present.
    fn contains_edge(&self, from: Self::NodeId, to: Self::NodeId) -> bool;
}

/// Directed-graph vocabulary: distinct outgoing and incoming neighbour
/// sets. Implementations preserve edge direction; calling
/// `outgoing(node)` returns successors, `incoming(node)` returns
/// predecessors.
///
/// Only types that genuinely encode direction implement this. An
/// [`Undirected`] graph cannot accidentally satisfy this trait by
/// returning the same set for both methods.
pub trait Directed: Graph {
    fn outgoing(&self, node: Self::NodeId) -> Vec<Self::NodeId>;
    fn incoming(&self, node: Self::NodeId) -> Vec<Self::NodeId>;
}

/// Undirected-graph vocabulary: a single neighbour set per node.
/// Calling `neighbors(node)` returns every node connected to `node` by
/// any edge, ignoring orientation. Implementations that genuinely lack
/// edge direction expose only this surface — directed callers must
/// require [`Directed`] explicitly.
pub trait Undirected: Graph {
    fn neighbors(&self, node: Self::NodeId) -> Vec<Self::NodeId>;
}

/// Node-keyed cascade operations: archive, unarchive, and hard-remove
/// every edge involving a given node.
///
/// Strictly mutating. Distinct from [`EdgeSet`] (which is read-only)
/// because the two surfaces are independent — a caller that just
/// counts edges doesn't need cascade authority, and a caller doing
/// soft-deletes doesn't need to ask for counts. Splitting them keeps
/// each trait's name accurate to its single purpose.
///
/// Generic over `Graph::NodeId` — no longer pinned to `Uuid`. The
/// kanban domain still uses `Uuid` for every edge today; a future
/// heterogeneous-entity graph can pick another identity (e.g. a
/// discriminated `(EntityKind, Uuid)` newtype) without touching this
/// trait or its callers.
pub trait Cascadable: Graph {
    /// Archive every edge involving `node` (soft delete).
    fn archive_node(&mut self, node: Self::NodeId);
    /// Unarchive the edges involving `node` whose other endpoint is live
    /// (per `is_live`). An edge to a still-archived neighbor stays tombstoned,
    /// preserving the born-archived invariant.
    fn unarchive_node(&mut self, node: Self::NodeId, is_live: &dyn Fn(Self::NodeId) -> bool);
    /// Remove every edge involving `node` (hard delete).
    fn remove_node(&mut self, node: Self::NodeId);
}

/// Set-like read access over a graph's edges: size, active-only size,
/// and membership.
///
/// Aligns with stdlib `HashSet` / `BTreeSet` vocabulary — `len`,
/// `contains` — so a reader who has seen those once knows the shape
/// of this trait immediately. Strictly read-only; lives separately
/// from [`Cascadable`] so a generic consumer can require only the
/// surface it actually uses, with no implicit mutation authority
/// leaking into inspection code.
///
/// Generic over `Graph::NodeId`, same reasoning as `Cascadable`.
///
/// # Active vs archived
///
/// `is_empty` and `contains` both consult the **active** subset only,
/// matching `Graph::contains_edge` and the principle of least
/// surprise: a caller asking "is this here right now?" wants the
/// current view, not the archive log. `contains_archived` is the
/// explicit escape hatch for callers that need to consult both. The
/// (active + archived) total is still available via `len`.
pub trait EdgeSet: Graph {
    /// Total edge count (active + archived).
    fn len(&self) -> usize;
    /// Active edge count only.
    fn active_len(&self) -> usize;
    /// True iff this set has no active edges. Archived edges, if any,
    /// are not counted: a graph whose entire active set has been
    /// archived reports `is_empty() == true`, matching what callers
    /// asking "anything here right now?" expect.
    fn is_empty(&self) -> bool {
        self.active_len() == 0
    }
    /// True iff an **active** edge between `a` and `b` exists in this
    /// set. Aligned with `Graph::contains_edge` semantics.
    fn contains(&self, a: Self::NodeId, b: Self::NodeId) -> bool;
    /// True iff any edge between `a` and `b` exists in this set,
    /// **including archived ones**. Use when reasoning about edge
    /// history; prefer `contains` for the current view.
    fn contains_archived(&self, a: Self::NodeId, b: Self::NodeId) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_trait_is_object_safe_for_uuid_nodes() {
        fn _accepts_dyn(_: &dyn Graph<NodeId = Uuid>) {}
    }

    #[test]
    fn test_directed_trait_is_object_safe() {
        fn _accepts_dyn(_: &dyn Directed<NodeId = Uuid>) {}
    }

    #[test]
    fn test_undirected_trait_is_object_safe() {
        fn _accepts_dyn(_: &dyn Undirected<NodeId = Uuid>) {}
    }

    #[test]
    fn test_cascadable_trait_is_object_safe() {
        fn _accepts_dyn(_: &dyn Cascadable<NodeId = Uuid>) {}
        fn _accepts_dyn_mut(_: &mut dyn Cascadable<NodeId = Uuid>) {}
    }

    #[test]
    fn test_edge_set_trait_is_object_safe() {
        fn _accepts_dyn(_: &dyn EdgeSet<NodeId = Uuid>) {}
    }
}
