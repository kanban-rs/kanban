use kanban_core::AppConfig;
use kanban_domain::KanbanResult;
use kanban_domain::{
    ArchivedCard, Board, BoardListFilter, BoardSortField, BoardUpdate, Card, CardListFilter,
    CardStatus, CardSummary, CardUpdate, Column, ColumnUpdate, CreateCardOptions, FieldUpdate,
    GraphOperations, Invalidation, KanbanOperations, NewColumn, SortOrder, Sprint, SprintUpdate,
};
use kanban_service::{AppType, KanbanContext, StoreManager};
use uuid::Uuid;

pub use kanban_service::BatchOperationResult;

pub struct CliContext {
    inner: KanbanContext,
    model: kanban_domain::Model,
    scope: crate::scope::CommandScope,
}

impl CliContext {
    /// The archival marker's `archived_at` for a card / board, or `None` if
    /// live. Lets `card get` / `board get` stamp the archived projection so an
    /// archived entity is never returned looking live.
    pub fn card_archived_at(
        &self,
        id: Uuid,
    ) -> KanbanResult<Option<chrono::DateTime<chrono::Utc>>> {
        self.inner.card_archived_at(id)
    }

    pub fn get_archived_card(&self, id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        self.inner.get_archived_card(id)
    }

    pub fn board_archived_at(
        &self,
        id: Uuid,
    ) -> KanbanResult<Option<chrono::DateTime<chrono::Utc>>> {
        self.inner.board_archived_at(id)
    }

    pub async fn load(
        store_manager: &StoreManager,
        file_path: &str,
        mut config: AppConfig,
    ) -> KanbanResult<Self> {
        if store_manager.sync_backend_with_file(file_path, &mut config) {
            eprintln!(
                "Warning: storage backend auto-corrected from config value to '{}' based on file content.",
                config.effective_storage_backend()
            );
        }
        let backend = store_manager.make_backend(file_path, &config).await?;
        Ok(Self {
            inner: KanbanContext::open(backend, config)
                .await?
                .with_app_type(AppType::Cli),
            model: kanban_domain::Model::default(),
            scope: crate::scope::CommandScope::default(),
        })
    }

    #[cfg(test)]
    pub(crate) fn from_context(inner: KanbanContext) -> Self {
        Self {
            inner,
            model: kanban_domain::Model::default(),
            scope: crate::scope::CommandScope::default(),
        }
    }

    pub(crate) fn set_scope(&mut self, scope: crate::scope::CommandScope) {
        self.scope = scope;
    }

    pub(crate) fn sync(&mut self) {
        self.inner.sync(
            &self.scope,
            &mut self.model,
            &mut kanban_domain::NoProjections,
        );
    }

    #[cfg(test)]
    pub(crate) fn model(&self) -> &kanban_domain::Model {
        &self.model
    }

    #[cfg(test)]
    pub(crate) fn model_mut(&mut self) -> &mut kanban_domain::Model {
        &mut self.model
    }

    /// The one place in `kanban-cli` where a mutation's `Invalidation` is
    /// handled. Every mutating handler goes through here; nothing in
    /// `handlers/` calls a `KanbanOperations` or `GraphOperations` mutator
    /// directly. The operation runs once; an `Err` propagates unchanged and
    /// invalidates nothing.
    pub(crate) fn mutate<T>(
        &mut self,
        op: impl FnOnce(&mut KanbanContext) -> KanbanResult<(T, Invalidation)>,
    ) -> KanbanResult<T> {
        let (value, invalidation) = op(&mut self.inner)?;
        self.inner.sync_invalidated(
            invalidation,
            &self.scope,
            &mut self.model,
            &mut kanban_domain::NoProjections,
        );
        Ok(value)
    }

    /// [`mutate`](Self::mutate) for the operations whose only result is the
    /// `Invalidation`.
    pub(crate) fn mutate_unit(
        &mut self,
        op: impl FnOnce(&mut KanbanContext) -> KanbanResult<Invalidation>,
    ) -> KanbanResult<()> {
        let invalidation = op(&mut self.inner)?;
        self.inner.sync_invalidated(
            invalidation,
            &self.scope,
            &mut self.model,
            &mut kanban_domain::NoProjections,
        );
        Ok(())
    }

    pub async fn save(&self) -> KanbanResult<()> {
        self.inner.save().await
    }

    /// Persist the default board-list sort through the service helper (R3):
    /// persist-first via `config::save`, no context rebuild. The canonical
    /// on-disk strings come from the domain `Display` (R1).
    pub fn set_board_sort(&mut self, field: BoardSortField, order: SortOrder) -> KanbanResult<()> {
        self.inner.set_board_sort(field, order)
    }

    /// The currently persisted live board-sort default, parsed from the held
    /// `AppConfig` via the domain canonical `FromStr` (R1). Any unset or
    /// unrecognized half falls back to [`DEFAULT_BOARD_SORT_LIVE`]. `set-sort`
    /// uses this to fill the half the caller did not pass so a partial update
    /// preserves the other dimension.
    pub fn effective_board_sort(&self) -> (BoardSortField, SortOrder) {
        use std::str::FromStr;
        let (default_field, default_order) = kanban_domain::DEFAULT_BOARD_SORT_LIVE;
        let config = self.inner.app_config();
        let field = config
            .board_sort_field
            .as_deref()
            .and_then(|s| BoardSortField::from_str(s).ok())
            .unwrap_or(default_field);
        let order = config
            .board_sort_order
            .as_deref()
            .and_then(|s| SortOrder::from_str(s).ok())
            .unwrap_or(default_order);
        (field, order)
    }

    pub fn execute_commands(
        &mut self,
        commands: Vec<kanban_domain::commands::Command>,
    ) -> KanbanResult<()> {
        self.inner.execute(commands).map(|_| ())
    }

    pub fn archive_cards_detailed(&mut self, ids: Vec<Uuid>) -> BatchOperationResult {
        self.inner.archive_cards_detailed(ids)
    }

    pub fn move_cards_detailed(&mut self, ids: Vec<Uuid>, column_id: Uuid) -> BatchOperationResult {
        self.inner.move_cards_detailed(ids, column_id)
    }

    /// Create a column carrying a `default_status`. An explicit `position`
    /// routes through the same `KanbanOperations::create_column` trait method
    /// `column create --position` already uses (not widened — it still takes
    /// no `default_status` parameter), followed by an `update_column` call
    /// that sets `default_status` on the freshly created column. A `None`
    /// position keeps the existing server-assigned append path via
    /// `create_column_from_spec`, which does carry `default_status` on the
    /// create spec itself.
    pub fn create_column_with_default_status(
        &mut self,
        board_id: Uuid,
        name: String,
        position: Option<i32>,
        default_status: Option<CardStatus>,
    ) -> KanbanResult<Column> {
        let column = match position {
            Some(position) => {
                let column = self.inner.create_column(board_id, name, Some(position))?;
                match default_status {
                    Some(_) => self.inner.update_column(
                        column.id,
                        ColumnUpdate {
                            name: None,
                            position: None,
                            wip_limit: FieldUpdate::NoChange,
                            default_status: Some(default_status),
                        },
                    )?,
                    None => column,
                }
            }
            None => {
                self.inner
                    .create_column_from_spec(
                        None,
                        NewColumn {
                            board_id,
                            name,
                            wip_limit: None,
                            default_status,
                        },
                    )?
                    .0
            }
        };
        Ok(column)
    }

    pub fn assign_cards_to_sprint_detailed(
        &mut self,
        ids: Vec<Uuid>,
        sprint_id: Uuid,
    ) -> BatchOperationResult {
        self.inner.assign_cards_to_sprint_detailed(ids, sprint_id)
    }
}

impl KanbanOperations for CliContext {
    fn create_board(&mut self, name: String, card_prefix: Option<String>) -> KanbanResult<Board> {
        self.inner.create_board(name, card_prefix)
    }

    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        self.inner.list_boards()
    }

    fn resolve_board_id(&self, raw: &str) -> KanbanResult<Uuid> {
        if let Ok(uuid) = Uuid::parse_str(raw) {
            return Ok(uuid);
        }
        let boards = crate::model_read::require_loaded(self.model.boards_state(), "board list")?;
        let matches = kanban_domain::find_boards_by_name(raw, boards);
        match matches.as_slice() {
            [] => Err(kanban_domain::KanbanError::not_found_by_name(
                "Board",
                raw,
                boards.iter().map(|b| b.name.clone()).collect(),
            )),
            [b] => Ok(b.id),
            many => Err(kanban_domain::KanbanError::ambiguous(
                "Board",
                raw,
                many.iter()
                    .map(|b| kanban_domain::AmbiguousMatch {
                        label: format!("'{}'", b.name),
                        id: b.id,
                    })
                    .collect(),
            )),
        }
    }

    fn list_boards_filtered(&self, filter: BoardListFilter) -> KanbanResult<Vec<Board>> {
        self.inner.list_boards_filtered(filter)
    }

    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        self.inner.get_board(id)
    }

    fn update_board(&mut self, id: Uuid, updates: BoardUpdate) -> KanbanResult<Board> {
        self.inner.update_board(id, updates)
    }

    fn delete_board(&mut self, id: Uuid) -> KanbanResult<()> {
        self.inner.delete_board(id)
    }
    fn archive_board(&mut self, id: Uuid) -> KanbanResult<()> {
        self.inner.archive_board(id)
    }
    fn restore_board(&mut self, id: Uuid) -> KanbanResult<()> {
        self.inner.restore_board(id)
    }
    fn list_archived_boards(&self) -> KanbanResult<Vec<kanban_domain::ArchivedBoard>> {
        self.inner.list_archived_boards()
    }

    fn create_column(
        &mut self,
        board_id: Uuid,
        name: String,
        position: Option<i32>,
    ) -> KanbanResult<Column> {
        self.inner.create_column(board_id, name, position)
    }

    fn list_columns(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        self.inner.list_columns(board_id)
    }

    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        self.inner.get_column(id)
    }

    fn update_column(&mut self, id: Uuid, updates: ColumnUpdate) -> KanbanResult<Column> {
        self.inner.update_column(id, updates)
    }

    fn delete_column(&mut self, id: Uuid) -> KanbanResult<()> {
        self.inner.delete_column(id)
    }

    fn reorder_column(&mut self, id: Uuid, new_position: i32) -> KanbanResult<Column> {
        self.inner.reorder_column(id, new_position)
    }

    fn create_card(
        &mut self,
        board_id: Uuid,
        column_id: Uuid,
        title: String,
        options: CreateCardOptions,
    ) -> KanbanResult<Card> {
        self.inner.create_card(board_id, column_id, title, options)
    }

    fn list_cards(&self, filter: CardListFilter) -> KanbanResult<Vec<CardSummary>> {
        self.inner.list_cards(filter)
    }

    fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        self.inner.get_card(id)
    }

    fn find_cards_by_identifier(&self, identifier: &str) -> KanbanResult<Vec<Card>> {
        self.inner.find_cards_by_identifier(identifier)
    }

    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        self.inner.list_all_cards()
    }

    fn list_all_columns(&self) -> KanbanResult<Vec<kanban_domain::Column>> {
        self.inner.list_all_columns()
    }

    fn list_all_sprints(&self) -> KanbanResult<Vec<kanban_domain::Sprint>> {
        self.inner.list_all_sprints()
    }

    fn update_card(&mut self, id: Uuid, updates: CardUpdate) -> KanbanResult<Card> {
        self.inner.update_card(id, updates)
    }

    fn move_card(
        &mut self,
        id: Uuid,
        column_id: Uuid,
        position: Option<i32>,
    ) -> KanbanResult<Card> {
        self.inner.move_card(id, column_id, position)
    }

    fn archive_card(&mut self, id: Uuid) -> KanbanResult<()> {
        self.inner.archive_card(id)
    }

    fn restore_card(&mut self, id: Uuid, column_id: Option<Uuid>) -> KanbanResult<Card> {
        self.inner.restore_card(id, column_id)
    }

    fn delete_card(&mut self, id: Uuid) -> KanbanResult<()> {
        self.inner.delete_card(id)
    }

    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.inner.list_archived_cards()
    }
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        self.inner.list_archived_cards_by_board(board_id)
    }

    fn assign_card_to_sprint(&mut self, card_id: Uuid, sprint_id: Uuid) -> KanbanResult<Card> {
        self.inner.assign_card_to_sprint(card_id, sprint_id)
    }

    fn unassign_card_from_sprint(&mut self, card_id: Uuid) -> KanbanResult<Card> {
        self.inner.unassign_card_from_sprint(card_id)
    }

    fn get_card_branch_name(&self, id: Uuid) -> KanbanResult<String> {
        self.inner.get_card_branch_name(id)
    }

    fn get_card_git_checkout(&self, id: Uuid) -> KanbanResult<String> {
        self.inner.get_card_git_checkout(id)
    }

    fn archive_cards(&mut self, ids: Vec<Uuid>) -> KanbanResult<usize> {
        self.inner.archive_cards(ids)
    }

    fn move_cards(&mut self, ids: Vec<Uuid>, column_id: Uuid) -> KanbanResult<usize> {
        self.inner.move_cards(ids, column_id)
    }

    fn update_cards(
        &mut self,
        updates: Vec<(Uuid, kanban_domain::CardUpdate)>,
    ) -> KanbanResult<usize> {
        self.inner.update_cards(updates)
    }

    fn assign_cards_to_sprint(&mut self, ids: Vec<Uuid>, sprint_id: Uuid) -> KanbanResult<usize> {
        self.inner.assign_cards_to_sprint(ids, sprint_id)
    }

    fn carry_over_sprint_cards(
        &mut self,
        from_sprint_id: Uuid,
        to_sprint_id: Uuid,
    ) -> KanbanResult<usize> {
        self.inner
            .carry_over_sprint_cards(from_sprint_id, to_sprint_id)
    }

    fn create_sprint(
        &mut self,
        board_id: Uuid,
        prefix: Option<String>,
        name: Option<String>,
    ) -> KanbanResult<Sprint> {
        self.inner.create_sprint(board_id, prefix, name)
    }

    fn list_sprints(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        self.inner.list_sprints(board_id)
    }

    fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        self.inner.get_sprint(id)
    }

    fn update_sprint(&mut self, id: Uuid, updates: SprintUpdate) -> KanbanResult<Sprint> {
        self.inner.update_sprint(id, updates)
    }

    fn activate_sprint(&mut self, id: Uuid, duration_days: Option<i32>) -> KanbanResult<Sprint> {
        self.inner.activate_sprint(id, duration_days)
    }

    fn complete_sprint(&mut self, id: Uuid) -> KanbanResult<Sprint> {
        self.inner.complete_sprint(id)
    }

    fn cancel_sprint(&mut self, id: Uuid) -> KanbanResult<Sprint> {
        self.inner.cancel_sprint(id)
    }

    fn delete_sprint(&mut self, id: Uuid) -> KanbanResult<()> {
        self.inner.delete_sprint(id)
    }

    fn export_board(&self, board_id: Option<Uuid>) -> KanbanResult<String> {
        self.inner.export_board(board_id)
    }

    fn import_board(&mut self, data: &str) -> KanbanResult<Board> {
        self.inner.import_board(data)
    }
}

impl GraphOperations for CliContext {
    fn attach_children(&mut self, parent: Uuid, children: Vec<Uuid>) -> KanbanResult<()> {
        self.inner.attach_children(parent, children)
    }
    fn detach_children(&mut self, parent: Uuid, children: Vec<Uuid>) -> KanbanResult<()> {
        self.inner.detach_children(parent, children)
    }
    fn list_children_of(&self, parent: Uuid) -> KanbanResult<Vec<Uuid>> {
        if self.inner.get_card(parent)?.is_none() {
            return Err(kanban_domain::KanbanError::not_found("Card", parent));
        }
        let graph =
            crate::model_read::require_loaded(self.model.graph_state(), "dependency graph")?;
        Ok(graph.children(parent))
    }
    fn list_parents_of(&self, child: Uuid) -> KanbanResult<Vec<Uuid>> {
        if self.inner.get_card(child)?.is_none() {
            return Err(kanban_domain::KanbanError::not_found("Card", child));
        }
        let graph =
            crate::model_read::require_loaded(self.model.graph_state(), "dependency graph")?;
        Ok(graph.parents(child))
    }
    fn block(
        &mut self,
        blocker: Uuid,
        blocked: Uuid,
        severity: kanban_domain::Severity,
    ) -> KanbanResult<()> {
        self.inner.block(blocker, blocked, severity)
    }
    fn unblock(&mut self, blocker: Uuid, blocked: Uuid) -> KanbanResult<()> {
        self.inner.unblock(blocker, blocked)
    }
    fn list_blocked_by(&self, blocker: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.inner.list_blocked_by(blocker)
    }
    fn list_blockers_of(&self, blocked: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.inner.list_blockers_of(blocked)
    }
    fn relate(&mut self, a: Uuid, b: Uuid, kind: kanban_domain::RelatesKind) -> KanbanResult<()> {
        self.inner.relate(a, b, kind)
    }
    fn dissociate(&mut self, a: Uuid, b: Uuid) -> KanbanResult<()> {
        self.inner.dissociate(a, b)
    }
    fn list_related_to(&self, card: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.inner.list_related_to(card)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{DomainError, KanbanError};

    fn seam_context() -> CliContext {
        CliContext::from_context(KanbanContext::open_deferred(
            std::sync::Arc::new(kanban_backend_memory::InMemoryStore::default()),
            AppConfig::default(),
        ))
    }

    #[test]
    fn test_mutate_returns_the_operations_value() -> KanbanResult<()> {
        let mut ctx = seam_context();
        let board = ctx.mutate(|c| c.create_board_impl("Seam".to_string(), None))?;
        assert_eq!(board.name, "Seam");
        assert_eq!(ctx.list_boards()?.len(), 1);
        Ok(())
    }

    #[test]
    fn test_mutate_propagates_the_error_and_invalidates_nothing() -> KanbanResult<()> {
        let mut ctx = seam_context();
        let missing_id = Uuid::new_v4();
        let result = ctx.mutate(|c| c.update_board_impl(missing_id, BoardUpdate::default()));
        match result {
            Err(KanbanError::Domain(DomainError::NotFound { entity, id })) => {
                assert_eq!(entity, "Board");
                assert_eq!(id, missing_id);
            }
            other => panic!("expected NotFound error, got {other:?}"),
        }
        assert!(ctx.list_boards()?.is_empty());
        Ok(())
    }

    #[test]
    fn test_mutate_runs_the_operation_exactly_once() -> KanbanResult<()> {
        let mut ctx = seam_context();
        let calls = std::cell::Cell::new(0usize);
        ctx.mutate(|c| {
            calls.set(calls.get() + 1);
            c.create_board_impl("Seam".to_string(), None)
        })?;
        assert_eq!(calls.get(), 1);
        assert_eq!(ctx.list_boards()?.len(), 1);
        Ok(())
    }

    #[test]
    fn test_mutate_unit_returns_unit_and_commits_the_operation() -> KanbanResult<()> {
        let mut ctx = seam_context();
        let board = ctx.mutate(|c| c.create_board_impl("Seam".to_string(), None))?;
        ctx.mutate_unit(|c| c.archive_board_impl(board.id))?;
        assert!(ctx.list_boards()?.is_empty());
        assert_eq!(ctx.list_archived_boards()?.len(), 1);
        Ok(())
    }

    #[test]
    fn test_mutate_unit_propagates_the_error_and_invalidates_nothing() -> KanbanResult<()> {
        let mut ctx = seam_context();
        let missing_id = Uuid::new_v4();
        let result = ctx.mutate_unit(|c| c.archive_board_impl(missing_id));
        match result {
            Err(KanbanError::Domain(DomainError::NotFound { entity, id })) => {
                assert_eq!(entity, "Board");
                assert_eq!(id, missing_id);
            }
            other => panic!("expected NotFound error, got {other:?}"),
        }
        assert!(ctx.list_archived_boards()?.is_empty());
        Ok(())
    }

    #[test]
    fn test_mutating_command_applies_the_returned_invalidation() -> KanbanResult<()> {
        use crate::cli::{BoardAction, BoardCommand, Commands};
        use crate::scope::CommandScope;

        let mut ctx = seam_context();
        ctx.mutate(|c| c.create_board_impl("First".to_string(), None))?;

        ctx.set_scope(CommandScope::from_command(&Commands::Board(BoardCommand {
            action: BoardAction::Get {
                board: "First".to_string(),
            },
        })));
        ctx.sync();
        assert_eq!(ctx.model().boards_state().loaded().unwrap().len(), 1);

        ctx.mutate(|c| c.create_board_impl("Second".to_string(), None))?;
        assert_eq!(ctx.model().boards_state().loaded().unwrap().len(), 2);

        Ok(())
    }

    #[test]
    fn test_relation_children_reads_the_graph_from_the_model_not_the_backend() -> KanbanResult<()> {
        use crate::cli::{Commands, RelationAction, RelationCommand, SortDir, SortKey};
        use crate::scope::CommandScope;

        let mut ctx = seam_context();
        let board = ctx.mutate(|c| c.create_board_impl("Board".to_string(), None))?;
        let column = ctx.mutate(|c| c.create_column_impl(board.id, "Col".to_string(), None))?;
        let parent = ctx.mutate(|c| {
            c.create_card_impl(
                board.id,
                column.id,
                "Parent".to_string(),
                kanban_domain::CreateCardOptions::default(),
            )
        })?;
        let child = ctx.mutate(|c| {
            c.create_card_impl(
                board.id,
                column.id,
                "Child".to_string(),
                kanban_domain::CreateCardOptions::default(),
            )
        })?;
        ctx.mutate_unit(|c| c.attach_children_impl(parent.id, vec![child.id]))?;

        assert!(ctx.list_children_of(parent.id).is_err());

        ctx.set_scope(CommandScope::from_command(&Commands::Relation(
            RelationCommand {
                action: RelationAction::Children {
                    card: parent.id.to_string(),
                    sort: SortKey::CardNumber,
                    order: SortDir::Asc,
                },
            },
        )));
        ctx.sync();
        assert_eq!(ctx.list_children_of(parent.id)?, vec![child.id]);

        Ok(())
    }
}
