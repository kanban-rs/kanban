use super::KanbanContext;
use kanban_core::graph::Edge;
use kanban_domain::commands::{
    AddBlocks, AddRelates, AddSpawns, Command, DependencyCommand, RemoveBlocks, RemoveRelates,
    RemoveSpawns,
};
use kanban_domain::{
    BlocksEdge, GraphOperations, KanbanError, KanbanResult, RelatesEdge, RelatesKind, Severity,
    SpawnsEdge,
};
use std::collections::HashSet;
use uuid::Uuid;

/// Active edges (not tombstoned) whose BOTH endpoints are in `members`.
/// Shared by the three edge kinds so the scope+liveness predicate lives once.
fn scoped_active_edges<E>(edges: &[E], members: &HashSet<Uuid>) -> Vec<E>
where
    E: Edge<NodeId = Uuid> + Clone,
{
    edges
        .iter()
        .filter(|e| e.is_active() && members.contains(&e.source()) && members.contains(&e.target()))
        .cloned()
        .collect()
}

/// The ACTIVE dependency edges whose BOTH endpoints belong to a single board's
/// cards (live + archived). The global [`kanban_domain::DependencyGraph`] is
/// keyed on card id with no board dimension, so board-scoping is the caller's
/// concern (see `GraphOperations::list_parents_of`); this bundle is that scoped
/// view, used to render a board's relations without leaking cross-board edges
/// (C10a). Tombstoned edges (soft-deleted by an incident card's archival) are
/// excluded, matching every other user-facing graph read
/// (`children`/`related`/`blocked` all traverse active edges only).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoardRelations {
    pub spawns: Vec<SpawnsEdge>,
    pub blocks: Vec<BlocksEdge>,
    pub relates: Vec<RelatesEdge>,
}

impl KanbanContext {
    /// All dependency edges internal to `board_id` (both endpoints among the
    /// board's live or archived cards). Works for a live OR an archived board.
    pub fn list_relations_for_board(&self, board_id: Uuid) -> KanbanResult<BoardRelations> {
        let col_ids: Vec<Uuid> = self
            .backend
            .list_columns_by_board(board_id)?
            .iter()
            .map(|c| c.id)
            .collect();
        let mut members: HashSet<Uuid> = self
            .backend
            .list_cards_by_columns(&col_ids)?
            .iter()
            .map(|c| c.id)
            .collect();
        // Include the board's archived cards so `members` is "all cards of the
        // board", independent of the archival model. Under [[KAN-864]] an archived
        // card is an ordinary card, so it CAN admit a (born-archived) edge; scoping
        // to the full card set keeps this read correct for those edges too.
        members.extend(
            self.backend
                .list_archived_cards_by_board(board_id)?
                .iter()
                .map(|ac| ac.entity_id),
        );

        let graph = self.backend.get_graph()?;
        Ok(BoardRelations {
            spawns: scoped_active_edges(graph.spawns_edges(), &members),
            blocks: scoped_active_edges(graph.blocks_edges(), &members),
            relates: scoped_active_edges(graph.relates_edges(), &members),
        })
    }
}

impl KanbanContext {
    /// Reject edge mutations against unknown card ids before the
    /// command reaches the graph. Without this guard a stale or
    /// fabricated UUID would silently land in the graph as a dangling
    /// edge — the CLI's identifier-resolution layer parses raw UUIDs
    /// without looking them up, so service-level enforcement is the
    /// right boundary.
    fn require_card_exists(&self, id: Uuid) -> KanbanResult<()> {
        match self.backend.get_card(id)? {
            Some(_) => Ok(()),
            None => Err(KanbanError::not_found("Card", id)),
        }
    }

    /// True if either endpoint is individually-archived. A new edge incident to
    /// an archived card is born archived so its state matches its endpoints
    /// ([[KAN-864]]/[[KAN-890]]): the card's pre-existing edges were archive-stamped,
    /// so a fresh ACTIVE edge would be inconsistent.
    fn edge_born_archived(&self, a: Uuid, b: Uuid) -> KanbanResult<bool> {
        Ok(self.backend.get_archived_card(a)?.is_some()
            || self.backend.get_archived_card(b)?.is_some())
    }
}

impl GraphOperations for KanbanContext {
    fn attach_children(&mut self, parent: Uuid, children: Vec<Uuid>) -> KanbanResult<()> {
        self.require_card_exists(parent)?;
        for child in &children {
            self.require_card_exists(*child)?;
        }
        let mut commands: Vec<Command> = Vec::with_capacity(children.len());
        for child in children {
            let as_archived = self.edge_born_archived(parent, child)?;
            commands.push(Command::Dependency(DependencyCommand::AddSpawns(
                AddSpawns {
                    source: parent,
                    target: child,
                    as_archived,
                },
            )));
        }
        self.execute(commands)
    }

    fn detach_children(&mut self, parent: Uuid, children: Vec<Uuid>) -> KanbanResult<()> {
        self.require_card_exists(parent)?;
        for child in &children {
            self.require_card_exists(*child)?;
        }
        let commands: Vec<Command> = children
            .into_iter()
            .map(|child| {
                Command::Dependency(DependencyCommand::RemoveSpawns(RemoveSpawns {
                    source: parent,
                    target: child,
                    tolerate_missing: false,
                }))
            })
            .collect();
        self.execute(commands)
    }

    fn list_children_of(&self, parent: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.require_card_exists(parent)?;
        Ok(self.backend.get_graph()?.children(parent))
    }

    fn list_parents_of(&self, child: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.require_card_exists(child)?;
        Ok(self.backend.get_graph()?.parents(child))
    }

    fn block(&mut self, blocker: Uuid, blocked: Uuid, severity: Severity) -> KanbanResult<()> {
        self.require_card_exists(blocker)?;
        self.require_card_exists(blocked)?;
        let as_archived = self.edge_born_archived(blocker, blocked)?;
        self.execute(vec![Command::Dependency(DependencyCommand::AddBlocks(
            AddBlocks {
                source: blocker,
                target: blocked,
                severity,
                as_archived,
            },
        ))])
    }

    fn unblock(&mut self, blocker: Uuid, blocked: Uuid) -> KanbanResult<()> {
        self.require_card_exists(blocker)?;
        self.require_card_exists(blocked)?;
        self.execute(vec![Command::Dependency(DependencyCommand::RemoveBlocks(
            RemoveBlocks {
                source: blocker,
                target: blocked,
                tolerate_missing: false,
            },
        ))])
    }

    fn list_blocked_by(&self, blocker: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.require_card_exists(blocker)?;
        Ok(self.backend.get_graph()?.blocked(blocker))
    }

    fn list_blockers_of(&self, blocked: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.require_card_exists(blocked)?;
        Ok(self.backend.get_graph()?.blockers(blocked))
    }

    fn relate(&mut self, a: Uuid, b: Uuid, kind: RelatesKind) -> KanbanResult<()> {
        self.require_card_exists(a)?;
        self.require_card_exists(b)?;
        let as_archived = self.edge_born_archived(a, b)?;
        self.execute(vec![Command::Dependency(DependencyCommand::AddRelates(
            AddRelates {
                source: a,
                target: b,
                kind,
                as_archived,
            },
        ))])
    }

    fn dissociate(&mut self, a: Uuid, b: Uuid) -> KanbanResult<()> {
        self.require_card_exists(a)?;
        self.require_card_exists(b)?;
        self.execute(vec![Command::Dependency(DependencyCommand::RemoveRelates(
            RemoveRelates {
                source: a,
                target: b,
                tolerate_missing: false,
            },
        ))])
    }

    fn list_related_to(&self, card: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.require_card_exists(card)?;
        Ok(self.backend.get_graph()?.related(card))
    }
}
