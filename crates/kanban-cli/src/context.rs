use kanban_core::AppConfig;
use kanban_domain::KanbanResult;
use kanban_domain::{
    ArchivedCard, Board, BoardListFilter, BoardSortField, BoardUpdate, Card, CardListFilter,
    CardSummary, CardUpdate, Column, ColumnUpdate, CreateCardOptions, GraphOperations,
    KanbanOperations, SortOrder, Sprint, SprintUpdate,
};
use kanban_service::{AppType, KanbanContext, StoreManager};
use uuid::Uuid;

pub use kanban_service::BatchOperationResult;

pub struct CliContext {
    inner: KanbanContext,
}

impl CliContext {
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
        })
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

    pub fn archive_cards_detailed(&mut self, ids: Vec<Uuid>) -> BatchOperationResult {
        self.inner.archive_cards_detailed(ids)
    }

    pub fn move_cards_detailed(&mut self, ids: Vec<Uuid>, column_id: Uuid) -> BatchOperationResult {
        self.inner.move_cards_detailed(ids, column_id)
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
        self.inner.list_children_of(parent)
    }
    fn list_parents_of(&self, child: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.inner.list_parents_of(child)
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
