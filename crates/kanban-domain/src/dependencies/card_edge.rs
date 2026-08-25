use serde::{Deserialize, Serialize};

/// Kind tag for cross-kind utilities and parameterised tests.
///
/// Production code paths are per-kind: each relation has its own
/// concrete edge struct ([`super::SpawnsEdge`] / [`super::BlocksEdge`]
/// / [`super::RelatesEdge`]), per-kind sub-graphs
/// (`DagGraph<SpawnsEdge>` etc.), per-kind GraphOperations verb pairs
/// (`attach_child(ren)` / `detach_child(ren)` for Spawns, `block` /
/// `unblock` for Blocks, `relate` / `dissociate` for Relates), and
/// per-kind DependencyCommand variants. This enum exists only where
/// a uniform discriminator is genuinely useful: cross-kind test
/// parameterisation, cross-kind debugging tools, and the
/// `requires_dag` / `allows_cycles` checks below.
///
/// Grammar is "source [variant] target": A `Blocks` B, A `RelatesTo`
/// B, A `Spawns` B.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CardEdgeType {
    /// This card blocks the target (must complete before target can start)
    /// Enforces DAG - no cycles allowed
    Blocks,

    /// General relationship (informational, allows cycles)
    RelatesTo,

    /// Parent/child edge: source spawns target as a sub-item, in a
    /// directed acyclic graph rather than a tree — target may have
    /// more than one parent. Transitive: if A spawns B and B spawns
    /// C, then A is an ancestor of C. Enforces DAG: no cycles allowed
    /// (a card can't be its own ancestor). The user-facing API still
    /// uses parent/child language (`set_parent`, `parents`,
    /// `children`) — that's how the relationship reads from either
    /// side; this variant names the directed edge itself.
    #[default]
    Spawns,
    // Future edge types can be added here:
    // Duplicates,
}

impl CardEdgeType {
    /// Whether this edge type enforces DAG (no cycles)
    pub fn requires_dag(&self) -> bool {
        matches!(self, CardEdgeType::Blocks | CardEdgeType::Spawns)
    }

    /// Whether this edge type allows cycles
    pub fn allows_cycles(&self) -> bool {
        !self.requires_dag()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blocks_requires_dag() {
        assert!(CardEdgeType::Blocks.requires_dag());
        assert!(!CardEdgeType::Blocks.allows_cycles());
    }

    #[test]
    fn test_relates_to_allows_cycles() {
        assert!(!CardEdgeType::RelatesTo.requires_dag());
        assert!(CardEdgeType::RelatesTo.allows_cycles());
    }

    #[test]
    fn test_spawns_requires_dag() {
        assert!(CardEdgeType::Spawns.requires_dag());
        assert!(!CardEdgeType::Spawns.allows_cycles());
    }
}
