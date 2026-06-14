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
/// - [`super::card_commands::DeleteCard::capture_inverse`]
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
        let mut board = context.get_board(self.board_id)?;
        let mut card = Card::new(
            &mut board,
            self.column_id,
            self.title.clone(),
            self.position,
        );
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
    /// edge added by the forward is cleaned up in the same step. The
    /// board's `card_counter` stays bumped; redo reproduces the same
    /// id and number.
    pub fn capture_inverse(&self, _store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        Ok(vec![Command::Card(
            super::card_commands::CardCommand::Delete(super::card_commands::DeleteCard {
                card_id: self.id,
            }),
        )])
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::TestContext;
    use super::*;
    use crate::DataStore;

    #[test]
    fn test_add_spawns_executes() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let parent_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();
        assert!(AddSpawns {
            source: parent_id,
            target: child_id,
            as_archived: false,
        }
        .execute(&context)
        .is_ok());
        let graph = tc.store.get_graph().unwrap();
        assert_eq!(graph.children(parent_id), vec![child_id]);
    }

    #[test]
    fn test_add_spawns_prevents_cycle() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        assert!(AddSpawns {
            source: a,
            target: b,
            as_archived: false,
        }
        .execute(&context)
        .is_ok());
        assert!(AddSpawns {
            source: b,
            target: a,
            as_archived: false,
        }
        .execute(&context)
        .is_err());
    }

    #[test]
    fn test_add_blocks_preserves_severity() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let blocker = Uuid::new_v4();
        let blocked = Uuid::new_v4();
        AddBlocks {
            source: blocker,
            target: blocked,
            severity: Severity::High,
            as_archived: false,
        }
        .execute(&context)
        .unwrap();
        let graph = tc.store.get_graph().unwrap();
        assert_eq!(graph.blocks_edges()[0].severity, Severity::High);
    }

    #[test]
    fn test_add_relates_preserves_kind() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        AddRelates {
            source: a,
            target: b,
            kind: RelatesKind::Duplicates,
            as_archived: false,
        }
        .execute(&context)
        .unwrap();
        let graph = tc.store.get_graph().unwrap();
        assert_eq!(graph.relates_edges()[0].kind, RelatesKind::Duplicates);
    }

    #[test]
    fn test_remove_spawns_executes() {
        let tc = TestContext::new();
        let parent_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();
        {
            let mut graph = tc.store.get_graph().unwrap();
            graph.set_parent(child_id, parent_id).unwrap();
            tc.store.set_graph(graph).unwrap();
        }
        let context = tc.as_command_context();
        assert!(RemoveSpawns {
            source: parent_id,
            target: child_id,
            tolerate_missing: false,
        }
        .execute(&context)
        .is_ok());
        let graph = tc.store.get_graph().unwrap();
        assert_eq!(graph.children(parent_id).len(), 0);
    }

    #[test]
    fn test_remove_blocks_inverse_captures_severity_from_pre_remove_graph() {
        let tc = TestContext::new();
        let blocker = Uuid::new_v4();
        let blocked = Uuid::new_v4();
        {
            let mut graph = tc.store.get_graph().unwrap();
            graph
                .set_block_with_severity(blocker, blocked, Severity::Critical)
                .unwrap();
            tc.store.set_graph(graph).unwrap();
        }
        let cmd = RemoveBlocks {
            source: blocker,
            target: blocked,
            tolerate_missing: false,
        };
        let inverse = cmd.capture_inverse(&tc.store).unwrap();
        match &inverse[0] {
            Command::Dependency(DependencyCommand::AddBlocks(a)) => {
                assert_eq!(a.severity, Severity::Critical);
            }
            other => panic!("expected AddBlocks inverse, got {other:?}"),
        }
    }

    #[test]
    fn test_remove_relates_inverse_captures_kind_from_pre_remove_graph() {
        let tc = TestContext::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        {
            let mut graph = tc.store.get_graph().unwrap();
            graph
                .relate_with_kind(a, b, RelatesKind::Duplicates)
                .unwrap();
            tc.store.set_graph(graph).unwrap();
        }
        let cmd = RemoveRelates {
            source: a,
            target: b,
            tolerate_missing: false,
        };
        let inverse = cmd.capture_inverse(&tc.store).unwrap();
        match &inverse[0] {
            Command::Dependency(DependencyCommand::AddRelates(a)) => {
                assert_eq!(a.kind, RelatesKind::Duplicates);
            }
            other => panic!("expected AddRelates inverse, got {other:?}"),
        }
    }

    /// Inverse round-trip across every Severity variant. A single-variant
    /// test (Critical above) doesn't catch the case where the impl
    /// silently casts or defaults a non-Default variant. Parameterised
    /// over Low / Medium / High / Critical — if any variant gets dropped
    /// the test fails on that variant.
    #[test]
    fn test_remove_blocks_inverse_preserves_severity_across_all_variants() {
        for severity in [
            Severity::Low,
            Severity::Medium,
            Severity::High,
            Severity::Critical,
        ] {
            let tc = TestContext::new();
            let blocker = Uuid::new_v4();
            let blocked = Uuid::new_v4();
            {
                let mut graph = tc.store.get_graph().unwrap();
                graph
                    .set_block_with_severity(blocker, blocked, severity)
                    .unwrap();
                tc.store.set_graph(graph).unwrap();
            }
            let cmd = RemoveBlocks {
                source: blocker,
                target: blocked,
                tolerate_missing: false,
            };
            let inverse = cmd.capture_inverse(&tc.store).unwrap();
            match &inverse[0] {
                Command::Dependency(DependencyCommand::AddBlocks(a)) => {
                    assert_eq!(
                        a.severity, severity,
                        "severity {severity:?} must round-trip through capture_inverse"
                    );
                }
                other => panic!("expected AddBlocks inverse for {severity:?}, got {other:?}"),
            }
        }
    }

    /// Same shape for RelatesKind. The undirected sub-graph means
    /// orientation matching also matters — see the dedicated test
    /// below.
    #[test]
    fn test_remove_relates_inverse_preserves_kind_across_all_variants() {
        for kind in [
            RelatesKind::General,
            RelatesKind::Duplicates,
            RelatesKind::MentionedIn,
        ] {
            let tc = TestContext::new();
            let a = Uuid::new_v4();
            let b = Uuid::new_v4();
            {
                let mut graph = tc.store.get_graph().unwrap();
                graph.relate_with_kind(a, b, kind).unwrap();
                tc.store.set_graph(graph).unwrap();
            }
            let cmd = RemoveRelates {
                source: a,
                target: b,
                tolerate_missing: false,
            };
            let inverse = cmd.capture_inverse(&tc.store).unwrap();
            match &inverse[0] {
                Command::Dependency(DependencyCommand::AddRelates(a)) => {
                    assert_eq!(
                        a.kind, kind,
                        "kind {kind:?} must round-trip through capture_inverse"
                    );
                }
                other => panic!("expected AddRelates inverse for {kind:?}, got {other:?}"),
            }
        }
    }

    /// `as_archived: true` on `AddSpawns` inserts the edge already in
    /// the archived state. Used by cascade-undo to preserve the
    /// archive state of incident edges across DeleteCard / undo
    /// cycles. Without this branch, restoring an archived incident
    /// edge as active would silently lose soft-delete history.
    #[test]
    fn test_add_spawns_with_as_archived_true_inserts_archived_edge() {
        use kanban_core::Edge as _;
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let parent = Uuid::new_v4();
        let child = Uuid::new_v4();
        AddSpawns {
            source: parent,
            target: child,
            as_archived: true,
        }
        .execute(&context)
        .unwrap();
        let graph = tc.store.get_graph().unwrap();
        // Active accessors must not see the edge.
        assert!(graph.children(parent).is_empty(), "active children empty");
        // Archived history must contain it.
        let edges = graph.spawns_edges();
        assert_eq!(edges.len(), 1, "edge present in history");
        assert!(!edges[0].is_active(), "edge is archived");
    }

    #[test]
    fn test_add_blocks_with_as_archived_true_inserts_archived_edge_with_severity() {
        use kanban_core::Edge as _;
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let blocker = Uuid::new_v4();
        let blocked = Uuid::new_v4();
        AddBlocks {
            source: blocker,
            target: blocked,
            severity: Severity::Critical,
            as_archived: true,
        }
        .execute(&context)
        .unwrap();
        let graph = tc.store.get_graph().unwrap();
        assert!(graph.blocked(blocker).is_empty(), "active blocked empty");
        let edges = graph.blocks_edges();
        assert_eq!(edges.len(), 1);
        assert!(!edges[0].is_active(), "edge archived");
        assert_eq!(
            edges[0].severity,
            Severity::Critical,
            "severity preserved through archived insert"
        );
    }

    #[test]
    fn test_add_relates_with_as_archived_true_inserts_archived_edge_with_kind() {
        use kanban_core::Edge as _;
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        AddRelates {
            source: a,
            target: b,
            kind: RelatesKind::Duplicates,
            as_archived: true,
        }
        .execute(&context)
        .unwrap();
        let graph = tc.store.get_graph().unwrap();
        assert!(graph.related(a).is_empty(), "active related empty");
        let edges = graph.relates_edges();
        assert_eq!(edges.len(), 1);
        assert!(!edges[0].is_active(), "edge archived");
        assert_eq!(
            edges[0].kind,
            RelatesKind::Duplicates,
            "kind preserved through archived insert"
        );
    }

    /// Backwards-compat: legacy command-log entries that pre-date the
    /// `as_archived` field deserialise with `as_archived = false`
    /// (active), matching the original semantics. `#[serde(default)]`
    /// keeps replay equivalence for all old logs.
    #[test]
    fn test_add_spawns_legacy_json_without_as_archived_defaults_to_false() {
        let source = Uuid::nil();
        let target = Uuid::from_u128(0x42);
        let legacy: DependencyCommand = serde_json::from_value(serde_json::json!({
            "action": "add_spawns",
            "source": source,
            "target": target,
        }))
        .expect("legacy add_spawns without as_archived must deserialise");
        match legacy {
            DependencyCommand::AddSpawns(a) => {
                assert!(
                    !a.as_archived,
                    "default must be active for backwards-compat"
                );
            }
            other => panic!("expected AddSpawns, got {other:?}"),
        }
    }

    /// RelatesEdge is undirected: if the user added with (a, b) but
    /// removes with (b, a), `RemoveRelates::capture_inverse` must still
    /// find the original edge and recover its kind. The impl uses
    /// either-orientation matching for exactly this reason — pinning
    /// the property protects it from a silent regression to
    /// directional-only matching.
    #[test]
    fn test_remove_relates_inverse_finds_edge_in_reversed_orientation() {
        let tc = TestContext::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        {
            let mut graph = tc.store.get_graph().unwrap();
            // Edge added in (a, b) orientation
            graph
                .relate_with_kind(a, b, RelatesKind::MentionedIn)
                .unwrap();
            tc.store.set_graph(graph).unwrap();
        }
        // Remove issued in (b, a) orientation
        let cmd = RemoveRelates {
            source: b,
            target: a,
            tolerate_missing: false,
        };
        let inverse = cmd.capture_inverse(&tc.store).unwrap();
        match &inverse[0] {
            Command::Dependency(DependencyCommand::AddRelates(restored)) => {
                assert_eq!(
                    restored.kind,
                    RelatesKind::MentionedIn,
                    "reversed-orientation remove must still recover the original kind"
                );
            }
            other => panic!("expected AddRelates inverse, got {other:?}"),
        }
    }

    /// Per-kind inverse: an AddSpawns undoes via a tolerant RemoveSpawns,
    /// not a kind-agnostic Remove. A `[AddSpawns(a,b), AddBlocks(a,b)]`
    /// batch's reverse-order undo now leaves each forward independently
    /// undone instead of having the first inverse wipe both kinds.
    #[test]
    fn test_add_spawns_inverse_is_tolerant_remove_spawns() {
        let tc = TestContext::new();
        let cmd = AddSpawns {
            source: Uuid::new_v4(),
            target: Uuid::new_v4(),
            as_archived: false,
        };
        let inverse = cmd.capture_inverse(&tc.store).unwrap();
        assert_eq!(inverse.len(), 1);
        match &inverse[0] {
            Command::Dependency(DependencyCommand::RemoveSpawns(r)) => {
                assert!(r.tolerate_missing, "undo inverse must tolerate missing");
            }
            other => panic!("expected RemoveSpawns inverse, got {other:?}"),
        }
    }

    #[test]
    fn test_add_blocks_inverse_is_tolerant_remove_blocks() {
        let tc = TestContext::new();
        let cmd = AddBlocks {
            source: Uuid::new_v4(),
            target: Uuid::new_v4(),
            severity: Severity::High,
            as_archived: false,
        };
        let inverse = cmd.capture_inverse(&tc.store).unwrap();
        assert_eq!(inverse.len(), 1);
        match &inverse[0] {
            Command::Dependency(DependencyCommand::RemoveBlocks(r)) => {
                assert!(r.tolerate_missing, "undo inverse must tolerate missing");
            }
            other => panic!("expected RemoveBlocks inverse, got {other:?}"),
        }
    }

    #[test]
    fn test_add_relates_inverse_is_tolerant_remove_relates() {
        let tc = TestContext::new();
        let cmd = AddRelates {
            source: Uuid::new_v4(),
            target: Uuid::new_v4(),
            kind: RelatesKind::Duplicates,
            as_archived: false,
        };
        let inverse = cmd.capture_inverse(&tc.store).unwrap();
        assert_eq!(inverse.len(), 1);
        match &inverse[0] {
            Command::Dependency(DependencyCommand::RemoveRelates(r)) => {
                assert!(r.tolerate_missing, "undo inverse must tolerate missing");
            }
            other => panic!("expected RemoveRelates inverse, got {other:?}"),
        }
    }

    /// `tolerate_missing = true` swallows EdgeNotFound; the
    /// user-initiated path (default `false`) propagates the error so
    /// the surface can render "no such edge to remove". This decouples
    /// replay tolerance from kind-agnosticism.
    #[test]
    fn test_remove_spawns_tolerant_succeeds_on_missing_edge() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let result = RemoveSpawns {
            source: Uuid::new_v4(),
            target: Uuid::new_v4(),
            tolerate_missing: true,
        }
        .execute(&context);
        assert!(result.is_ok(), "tolerant remove must swallow EdgeNotFound");
    }

    #[test]
    fn test_remove_spawns_strict_errors_on_missing_edge() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let result = RemoveSpawns {
            source: Uuid::new_v4(),
            target: Uuid::new_v4(),
            tolerate_missing: false,
        }
        .execute(&context);
        assert!(
            result.unwrap_err().is_edge_not_found(),
            "strict remove must propagate EdgeNotFound"
        );
    }

    #[test]
    fn test_remove_blocks_tolerant_succeeds_on_missing_edge() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let result = RemoveBlocks {
            source: Uuid::new_v4(),
            target: Uuid::new_v4(),
            tolerate_missing: true,
        }
        .execute(&context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_remove_relates_tolerant_succeeds_on_missing_edge() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let result = RemoveRelates {
            source: Uuid::new_v4(),
            target: Uuid::new_v4(),
            tolerate_missing: true,
        }
        .execute(&context);
        assert!(result.is_ok());
    }

    /// Pin the on-disk JSON shape of `DependencyCommand` variants so a
    /// future SQLite command-log wiring (the schema exists today but
    /// the writer is dormant) treats the collapsed variant names as a
    /// backwards-compatibility contract. Any rename or reshape will
    /// fail this test loudly rather than silently breaking replay.
    #[test]
    fn test_dependency_command_serialization_shape_is_stable() {
        let source = Uuid::nil();
        let target = Uuid::from_u128(0x42);

        // Per-kind add variants
        let add_spawns = DependencyCommand::AddSpawns(AddSpawns {
            source,
            target,
            as_archived: false,
        });
        let json = serde_json::to_value(&add_spawns).unwrap();
        assert_eq!(json["action"], "add_spawns");

        let add_blocks = DependencyCommand::AddBlocks(AddBlocks {
            source,
            target,
            severity: Severity::High,
            as_archived: false,
        });
        let json = serde_json::to_value(&add_blocks).unwrap();
        assert_eq!(json["action"], "add_blocks");
        assert_eq!(json["severity"], "High");

        let add_relates = DependencyCommand::AddRelates(AddRelates {
            source,
            target,
            kind: RelatesKind::Duplicates,
            as_archived: false,
        });
        let json = serde_json::to_value(&add_relates).unwrap();
        assert_eq!(json["action"], "add_relates");
        assert_eq!(json["kind"], "Duplicates");

        // Per-kind remove variants
        let remove_blocks = DependencyCommand::RemoveBlocks(RemoveBlocks {
            source,
            target,
            tolerate_missing: false,
        });
        let json = serde_json::to_value(&remove_blocks).unwrap();
        assert_eq!(json["action"], "remove_blocks");
        assert_eq!(json["tolerate_missing"], false);

        // Backwards-compat: pre-tolerance JSON (no `tolerate_missing`
        // field) deserialises with `tolerate_missing = false` via
        // `#[serde(default)]`. Old command-log entries stay valid.
        let legacy: DependencyCommand = serde_json::from_value(serde_json::json!({
            "action": "remove_spawns",
            "source": source,
            "target": target
        }))
        .expect("legacy remove_spawns without tolerate_missing must deserialise");
        match legacy {
            DependencyCommand::RemoveSpawns(r) => {
                assert!(!r.tolerate_missing, "default must be strict");
            }
            other => panic!("expected RemoveSpawns, got {other:?}"),
        }

        // Round-trip
        let round: DependencyCommand =
            serde_json::from_value(serde_json::to_value(&add_blocks).unwrap()).unwrap();
        assert!(matches!(
            round,
            DependencyCommand::AddBlocks(AddBlocks {
                severity: Severity::High,
                ..
            })
        ));
    }

    #[test]
    fn test_create_subcard_command() {
        use crate::Board;

        let tc = TestContext::new();
        let column_id = Uuid::new_v4();

        let mut board = Board::new("Test Board", None::<String>);
        board.card_prefix = Some("TEST".to_string());
        let board_id = board.id;
        let parent = crate::Card::new(&mut board, column_id, "Parent", 0);
        let parent_id = parent.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_card(parent).unwrap();

        let context = tc.as_command_context();
        let cmd = CreateSubcardCommand {
            id: Uuid::new_v4(),
            parent_id,
            board_id,
            column_id,
            title: "Test Subcard".to_string(),
            description: Some("Test description".to_string()),
            position: 0,
        };

        assert!(cmd.execute(&context).is_ok());

        let cards = tc.store.list_all_cards().unwrap();
        assert_eq!(cards.len(), 2);
        let card = cards.iter().find(|c| c.title == "Test Subcard").unwrap();
        assert_eq!(card.description, Some("Test description".to_string()));
        assert_eq!(card.column_id, column_id);

        let graph = tc.store.get_graph().unwrap();
        assert_eq!(graph.children(parent_id).len(), 1);
        assert!(graph.children(parent_id).contains(&card.id));
    }

    #[test]
    fn test_create_subcard_with_nonexistent_parent_returns_not_found() {
        let tc = TestContext::new();
        let board = crate::Board::new("B", Some("TST"));
        let col = crate::Column::new(board.id, "Col", 0);
        let board_id = board.id;
        let column_id = col.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();

        let context = tc.as_command_context();
        let cmd = CreateSubcardCommand {
            id: Uuid::new_v4(),
            parent_id: Uuid::new_v4(),
            board_id,
            column_id,
            title: "Subcard".to_string(),
            description: None,
            position: 0,
        };
        let result = cmd.execute(&context);
        assert!(result.is_err(), "Expected error for nonexistent parent");
        assert!(result.unwrap_err().is_not_found());
    }
}
