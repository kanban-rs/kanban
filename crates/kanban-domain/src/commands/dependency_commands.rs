use crate::data_store::DataStore;
use crate::dependencies::{RelatesKind, Severity};
use crate::KanbanResult;
use kanban_core::Edge as _;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Command, CommandContext};
use crate::Card;

/// Per-kind dependency commands.
///
/// Each variant has a single relation kind baked into its type and
/// carries the kind-specific metadata (severity on Blocks, kind on
/// Relates) directly. No runtime kind discriminator: replay sees
/// the same metadata the forward saw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum DependencyCommand {
    AddSpawns(AddSpawns),
    AddBlocks(AddBlocks),
    AddRelates(AddRelates),
    RemoveSpawns(RemoveSpawns),
    RemoveBlocks(RemoveBlocks),
    RemoveRelates(RemoveRelates),
    /// Atomic create-card-and-link-as-subcard. Genuinely different
    /// from the edge commands — touches the board (card counter), the
    /// card store (new card), and the graph (parent edge). Its
    /// inverse is `DeleteCard` (polymorphic over live/archived, also
    /// strips incident edges).
    CreateSubcard(CreateSubcardCommand),
}

impl DependencyCommand {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        match self {
            DependencyCommand::AddSpawns(c) => c.execute(context),
            DependencyCommand::AddBlocks(c) => c.execute(context),
            DependencyCommand::AddRelates(c) => c.execute(context),
            DependencyCommand::RemoveSpawns(c) => c.execute(context),
            DependencyCommand::RemoveBlocks(c) => c.execute(context),
            DependencyCommand::RemoveRelates(c) => c.execute(context),
            DependencyCommand::CreateSubcard(c) => c.execute(context),
        }
    }

    pub fn description(&self) -> String {
        match self {
            DependencyCommand::AddSpawns(c) => c.description(),
            DependencyCommand::AddBlocks(c) => c.description(),
            DependencyCommand::AddRelates(c) => c.description(),
            DependencyCommand::RemoveSpawns(c) => c.description(),
            DependencyCommand::RemoveBlocks(c) => c.description(),
            DependencyCommand::RemoveRelates(c) => c.description(),
            DependencyCommand::CreateSubcard(c) => c.description(),
        }
    }

    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        match self {
            DependencyCommand::AddSpawns(c) => c.capture_inverse(store),
            DependencyCommand::AddBlocks(c) => c.capture_inverse(store),
            DependencyCommand::AddRelates(c) => c.capture_inverse(store),
            DependencyCommand::RemoveSpawns(c) => c.capture_inverse(store),
            DependencyCommand::RemoveBlocks(c) => c.capture_inverse(store),
            DependencyCommand::RemoveRelates(c) => c.capture_inverse(store),
            DependencyCommand::CreateSubcard(c) => c.capture_inverse(store),
        }
    }
}

// ────────────────────────────────────────────────────────────────────
// Add* commands: per-kind, carry the kind-specific metadata.
// ────────────────────────────────────────────────────────────────────

/// Add a parent->child Spawns edge. `source` is the parent,
/// `target` is the child.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddSpawns {
    pub source: Uuid,
    pub target: Uuid,
    /// When `true`, insert the edge already in the archived state.
    /// Used by cascade-undo (`DeleteCard` / `DeleteCardEdges`) to
    /// preserve the archive state of incident edges across delete/undo
    /// cycles. User-initiated `attach_child(ren)` paths leave this
    /// `false` (default) so edges land active.
    ///
    /// `#[serde(default)]` lets legacy command-log entries (pre-fix)
    /// deserialise with `false`, matching their original semantics.
    #[serde(default)]
    pub as_archived: bool,
}

impl AddSpawns {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let (source, target, as_archived) = (self.source, self.target, self.as_archived);
        context.store.modify_graph(Box::new(move |graph| {
            if as_archived {
                graph.add_archived_spawns(source, target)
            } else {
                graph.set_parent(target, source)
            }
        }))
    }

    pub fn description(&self) -> String {
        format!("Set parent: {} is parent of {}", self.source, self.target)
    }

    /// Inverse: per-kind [`RemoveSpawns`] with `tolerate_missing =
    /// true` so undo replay succeeds even if intervening state has
    /// already removed the edge. Per-kind tolerance keeps the inverse
    /// in the same edge kind as the forward — a `[AddSpawns(a,b),
    /// AddBlocks(a,b)]` batch now undoes each edge independently
    /// instead of having the first inverse wipe both kinds.
    pub fn capture_inverse(&self, _store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        Ok(vec![Command::Dependency(DependencyCommand::RemoveSpawns(
            RemoveSpawns {
                source: self.source,
                target: self.target,
                tolerate_missing: true,
            },
        ))])
    }
}

/// Add a blocker->blocked Blocks edge with a severity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddBlocks {
    pub source: Uuid,
    pub target: Uuid,
    #[serde(default)]
    pub severity: Severity,
    /// See [`AddSpawns::as_archived`] for the cascade-undo rationale.
    #[serde(default)]
    pub as_archived: bool,
}

impl AddBlocks {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let (source, target, severity, as_archived) =
            (self.source, self.target, self.severity, self.as_archived);
        context.store.modify_graph(Box::new(move |graph| {
            if as_archived {
                graph.add_archived_blocks(source, target, severity)
            } else {
                graph.set_block_with_severity(source, target, severity)
            }
        }))
    }

    pub fn description(&self) -> String {
        format!(
            "Add blocks dependency ({:?}): {} blocks {}",
            self.severity, self.source, self.target
        )
    }

    /// Inverse: per-kind [`RemoveBlocks`] with `tolerate_missing =
    /// true`. See [`AddSpawns::capture_inverse`] for the rationale.
    pub fn capture_inverse(&self, _store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        Ok(vec![Command::Dependency(DependencyCommand::RemoveBlocks(
            RemoveBlocks {
                source: self.source,
                target: self.target,
                tolerate_missing: true,
            },
        ))])
    }
}

/// Add an undirected RelatesTo edge with a sub-kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddRelates {
    pub source: Uuid,
    pub target: Uuid,
    #[serde(default)]
    pub kind: RelatesKind,
    /// See [`AddSpawns::as_archived`] for the cascade-undo rationale.
    #[serde(default)]
    pub as_archived: bool,
}

impl AddRelates {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let (source, target, kind, as_archived) =
            (self.source, self.target, self.kind, self.as_archived);
        context.store.modify_graph(Box::new(move |graph| {
            if as_archived {
                graph.add_archived_relates(source, target, kind)
            } else {
                graph.relate_with_kind(source, target, kind)
            }
        }))
    }

    pub fn description(&self) -> String {
        format!(
            "Add relates-to dependency ({:?}): {} <-> {}",
            self.kind, self.source, self.target
        )
    }

    /// Inverse: per-kind [`RemoveRelates`] with `tolerate_missing =
    /// true`. See [`AddSpawns::capture_inverse`] for the rationale.
    pub fn capture_inverse(&self, _store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        Ok(vec![Command::Dependency(DependencyCommand::RemoveRelates(
            RemoveRelates {
                source: self.source,
                target: self.target,
                tolerate_missing: true,
            },
        ))])
    }
}

// ────────────────────────────────────────────────────────────────────
// Remove* commands: per-kind. `tolerate_missing` decouples the
// undo-replay tolerance from kind-agnosticism. Edges are identified
// by (kind, source, target); the kind comes from the variant, so
// metadata fields stay scoped to add commands.
// ────────────────────────────────────────────────────────────────────

/// Remove a parent->child Spawns edge.
///
/// `tolerate_missing` controls behavior when the edge is absent at
/// execute time:
/// - `false` (default, user-initiated paths): returns
///   [`DependencyError::EdgeNotFound`] so the surface can render
///   "no such edge to remove" to the user.
/// - `true` (inverse-replay paths): swallows `EdgeNotFound` and
///   returns `Ok(())`. The undo invariant requires inverses to
///   succeed even if intervening state has already removed the edge.
///
/// The flag decouples *tolerance* (a replay concern) from
/// *kind-agnosticism* (a separate dimension). Each per-kind remove
/// stays in its own kind and chooses its tolerance at construction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoveSpawns {
    pub source: Uuid,
    pub target: Uuid,
    #[serde(default)]
    pub tolerate_missing: bool,
}

impl RemoveSpawns {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let (source, target, tolerate) = (self.source, self.target, self.tolerate_missing);
        context.store.modify_graph(Box::new(move |graph| {
            match graph.remove_parent(target, source) {
                Ok(()) => Ok(()),
                Err(e) if tolerate && e.is_edge_not_found() => Ok(()),
                Err(e) => Err(e),
            }
        }))
    }

    pub fn description(&self) -> String {
        format!(
            "Remove parent: {} is no longer parent of {}",
            self.source, self.target
        )
    }

    /// Inverse: re-add the parent edge (as active — user-initiated
    /// removes only fire against active edges, so the original state
    /// was active).
    pub fn capture_inverse(&self, _store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        Ok(vec![Command::Dependency(DependencyCommand::AddSpawns(
            AddSpawns {
                source: self.source,
                target: self.target,
                as_archived: false,
            },
        ))])
    }
}

/// Remove a blocker->blocked Blocks edge. See [`RemoveSpawns`] for the
/// `tolerate_missing` flag semantics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoveBlocks {
    pub source: Uuid,
    pub target: Uuid,
    #[serde(default)]
    pub tolerate_missing: bool,
}

impl RemoveBlocks {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let (source, target, tolerate) = (self.source, self.target, self.tolerate_missing);
        context
            .store
            .modify_graph(Box::new(move |graph| match graph.unblock(source, target) {
                Ok(()) => Ok(()),
                Err(e) if tolerate && e.is_edge_not_found() => Ok(()),
                Err(e) => Err(e),
            }))
    }

    pub fn description(&self) -> String {
        format!(
            "Remove blocks dependency: {} no longer blocks {}",
            self.source, self.target
        )
    }

    /// Inverse: re-add the blocks edge. We don't know the original
    /// severity at remove time; the capture function walks the
    /// pre-remove graph to record it.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let graph = store.get_graph()?;
        let severity = graph
            .blocks_edges()
            .iter()
            .find(|e| e.source() == self.source && e.target() == self.target)
            .map(|e| e.severity)
            .unwrap_or_default();
        Ok(vec![Command::Dependency(DependencyCommand::AddBlocks(
            AddBlocks {
                source: self.source,
                target: self.target,
                severity,
                as_archived: false,
            },
        ))])
    }
}

/// Remove an undirected RelatesTo edge. See [`RemoveSpawns`] for the
/// `tolerate_missing` flag semantics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoveRelates {
    pub source: Uuid,
    pub target: Uuid,
    #[serde(default)]
    pub tolerate_missing: bool,
}

impl RemoveRelates {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let (source, target, tolerate) = (self.source, self.target, self.tolerate_missing);
        context.store.modify_graph(Box::new(move |graph| {
            match graph.dissociate(source, target) {
                Ok(()) => Ok(()),
                Err(e) if tolerate && e.is_edge_not_found() => Ok(()),
                Err(e) => Err(e),
            }
        }))
    }

    pub fn description(&self) -> String {
        format!(
            "Remove relates-to dependency: {} <-> {}",
            self.source, self.target
        )
    }

    /// Inverse: re-add the relates edge. Same as RemoveBlocks: we
    /// capture the kind from the pre-remove graph.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let graph = store.get_graph()?;
        let (a, b) = (self.source, self.target);
        let kind = graph
            .relates_edges()
            .iter()
            .find(|e| (e.source() == a && e.target() == b) || (e.source() == b && e.target() == a))
            .map(|e| e.kind)
            .unwrap_or_default();
        Ok(vec![Command::Dependency(DependencyCommand::AddRelates(
            AddRelates {
                source: self.source,
                target: self.target,
                kind,
                as_archived: false,
            },
        ))])
    }
}

// ────────────────────────────────────────────────────────────────────
// Inverse-replay helper.
// ────────────────────────────────────────────────────────────────────

/// Build inverse-replay `Add*` commands for every edge in `graph`
/// that matches `predicate`. Each per-kind sub-graph contributes its
/// matching edges with metadata (severity / kind) and archive state
/// preserved. Archived edges restore as archived; active edges restore
/// as active. Without this distinction, cascade-undo silently revived
/// archived incident edges to active state — losing the soft-delete
/// history that `archive_node` had recorded.
///
/// Used by the cascade capture-inverse sites that need to restore
/// edges of every kind touching one or more nodes:
/// - [`super::cascade_commands::DeleteCardEdges::capture_inverse`]
/// - [`super::card::DeleteCard::capture_inverse`]
pub(super) fn edges_to_undo_commands<P>(
    graph: &crate::DependencyGraph,
    predicate: P,
) -> Vec<Command>
where
    P: Fn(Uuid, Uuid) -> bool,
{
    use kanban_core::Edge as _;
    let mut out = Vec::new();
    for e in graph.spawns_edges() {
        if predicate(e.source(), e.target()) {
            out.push(Command::Dependency(DependencyCommand::AddSpawns(
                AddSpawns {
                    source: e.source(),
                    target: e.target(),
                    as_archived: !e.is_active(),
                },
            )));
        }
    }
    for e in graph.blocks_edges() {
        if predicate(e.source(), e.target()) {
            out.push(Command::Dependency(DependencyCommand::AddBlocks(
                AddBlocks {
                    source: e.source(),
                    target: e.target(),
                    severity: e.severity,
                    as_archived: !e.is_active(),
                },
            )));
        }
    }
    for e in graph.relates_edges() {
        if predicate(e.source(), e.target()) {
            out.push(Command::Dependency(DependencyCommand::AddRelates(
                AddRelates {
                    source: e.source(),
                    target: e.target(),
                    kind: e.kind,
                    as_archived: !e.is_active(),
                },
            )));
        }
    }
    out
}

/// Create a new card as a subcard of a parent card
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateSubcardCommand {
    /// Stable id for the new subcard, baked in at construction so undo
    /// (KAN-191) can target a DeleteCard at the right id without needing
    /// to read post-execute state.
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub parent_id: Uuid,
    pub board_id: Uuid,
    pub column_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub position: i32,
}

impl CreateSubcardCommand {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        context.get_card(self.parent_id)?;
        let board = context.get_board(self.board_id)?;
        // Allocates from the prefix row, like every other card. Minting from
        // `board.card_counter` here would draw subcards from a different
        // counter than their siblings and collide with them.
        let (prefix, card_number) = crate::prefix::allocate_card_number(
            context.store,
            board.card_prefix.as_deref(),
            None,
            crate::prefix_backfill::DEFAULT_CARD_PREFIX,
        )?;
        let mut card = Card::new(board.id, self.column_id, self.title.clone(), self.position);
        card.card_number = card_number;
        card.prefix = prefix;
        card.id = self.id;

        if let Some(desc) = &self.description {
            card.description = Some(desc.clone());
        }

        let card_id = card.id;
        let parent_id = self.parent_id;
        context.store.upsert_board(board)?;
        context.store.upsert_card(card)?;

        context
            .store
            .modify_graph(Box::new(move |graph| graph.set_parent(card_id, parent_id)))
    }

    pub fn description(&self) -> String {
        format!(
            "Create subcard '{}' under parent {}",
            self.title, self.parent_id
        )
    }

    /// Inverse: delete the new card. `DeleteCard` is polymorphic over
    /// live / archived and strips incident graph edges, so the parent
    /// edge added by the forward is cleaned up in the same step. Redo
    /// reproduces the same id and number.
    pub fn capture_inverse(&self, _store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        Ok(vec![Command::Card(super::card::CardCommand::Delete(
            super::card::DeleteCard { card_id: self.id },
        ))])
    }
}
