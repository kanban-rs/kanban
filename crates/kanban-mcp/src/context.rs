use kanban_core::{AppConfig, PaginatedList};
use kanban_domain::KanbanResult;
use kanban_domain::{
    ArchivedCard, Board, BoardUpdate, Card, CardListFilter, CardSummary, CardUpdate, Column,
    ColumnUpdate, CreateCardOptions, GraphOperations, KanbanOperations, Sprint, SprintUpdate,
};
use kanban_service::{AppType, KanbanContext, StoreManager};
use uuid::Uuid;

pub struct McpContext {
    inner: KanbanContext,
}

impl McpContext {
    pub async fn new(
        store_manager: &StoreManager,
        data_file: &str,
        mut config: AppConfig,
    ) -> KanbanResult<Self> {
        if store_manager.sync_backend_with_file(data_file, &mut config) {
            tracing::warn!(
                "Storage backend auto-corrected to '{}' based on file content.",
                config.effective_storage_backend()
            );
        }
        let backend = store_manager.make_backend(data_file, &config).await?;
        Ok(Self {
            inner: KanbanContext::open(backend, config)
                .await?
                .with_app_type(AppType::Mcp),
        })
    }

    pub async fn reload(&mut self) -> KanbanResult<()> {
        self.inner.reload().await
    }

    pub fn clear_history(&mut self) -> KanbanResult<()> {
        self.inner.clear_history()
    }

    pub fn undo(&mut self) -> KanbanResult<bool> {
        self.inner.undo()
    }

    pub fn redo(&mut self) -> KanbanResult<bool> {
        self.inner.redo()
    }

    pub fn can_undo(&self) -> bool {
        self.inner.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.inner.can_redo()
    }

    pub async fn save(&self) -> KanbanResult<()> {
        self.inner.save().await
    }

    /// MCP-specific method that exposes pagination.
    /// `KanbanOperations::list_cards` cannot carry pagination params, so
    /// `tool_list_cards` calls this directly.
    pub fn list_cards_paged(
        &self,
        filter: CardListFilter,
        page: usize,
        page_size: usize,
    ) -> KanbanResult<PaginatedList<CardSummary>> {
        let cards = self.inner.list_cards(filter)?;
        Ok(PaginatedList::paginate(cards, page, page_size)?)
    }
}

impl KanbanOperations for McpContext {
    // ========================================================================
    // Board Operations
    // ========================================================================

    fn create_board(&mut self, name: String, card_prefix: Option<String>) -> KanbanResult<Board> {
        self.inner.create_board(name, card_prefix)
    }

    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        self.inner.list_boards()
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

    // ========================================================================
    // Column Operations
    // ========================================================================

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

    // ========================================================================
    // Card Operations
    // ========================================================================

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

    // ========================================================================
    // Card Sprint Operations
    // ========================================================================

    fn assign_card_to_sprint(&mut self, card_id: Uuid, sprint_id: Uuid) -> KanbanResult<Card> {
        self.inner.assign_card_to_sprint(card_id, sprint_id)
    }

    fn unassign_card_from_sprint(&mut self, card_id: Uuid) -> KanbanResult<Card> {
        self.inner.unassign_card_from_sprint(card_id)
    }

    // ========================================================================
    // Card Utilities
    // ========================================================================

    fn get_card_branch_name(&self, id: Uuid) -> KanbanResult<String> {
        self.inner.get_card_branch_name(id)
    }

    fn get_card_git_checkout(&self, id: Uuid) -> KanbanResult<String> {
        self.inner.get_card_git_checkout(id)
    }

    // ========================================================================
    // Multi-card operations
    // ========================================================================

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

    // ========================================================================
    // Sprint Operations
    // ========================================================================

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

    // ========================================================================
    // Import/Export
    // ========================================================================

    fn export_board(&self, board_id: Option<Uuid>) -> KanbanResult<String> {
        self.inner.export_board(board_id)
    }

    fn import_board(&mut self, data: &str) -> KanbanResult<Board> {
        self.inner.import_board(data)
    }
}

impl GraphOperations for McpContext {
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
