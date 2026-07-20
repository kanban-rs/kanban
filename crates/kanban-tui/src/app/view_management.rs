use super::{App, AppMode};
use crate::view_strategy::{UnifiedViewStrategy, ViewRefreshContext, ViewStrategy};
use kanban_domain::{Board, Card, KanbanResult};

impl App {
    /// Resolve the board the user is currently acting on / viewing, by identity.
    /// This is the single, archival-agnostic active-board accessor every
    /// operation and view uses: a board is a board whether its head is live or
    /// archived. Returns `None` when no board is active (browsing the projects
    /// list without opening one).
    ///
    /// Note: `prepare_frame` inlines the equivalent resolution instead of calling
    /// this helper because it also needs a mutable borrow of `self.view.strategy`
    /// in the same scope, which Rust's NLL cannot split through a method boundary.
    /// All other call sites use this helper directly.
    pub fn active_board(&self) -> Option<&Board> {
        self.selection
            .active_board_id
            .and_then(|id| self.model.board_by_id(id))
    }

    /// The board a board-scoped operation acts on: the active board (by id) if
    /// one is open, otherwise the board highlighted in the currently displayed
    /// projects set. Board-agnostic — resolves live OR archived boards uniformly,
    /// so board detail, sprints, and columns work identically for either.
    pub fn board_in_context(&self) -> Option<&Board> {
        match self.selection.active_board_id {
            Some(id) => self.model.board_by_id(id),
            None => self
                .selection
                .board
                .get()
                .and_then(|idx| self.displayed_boards().get(idx)),
        }
    }

    /// The board set the projects panel currently displays: the archived heads
    /// when the panel is toggled to the archived set, the live boards otherwise.
    /// This is the SOLE place the live/archived distinction affects behavior —
    /// the projects panel choosing which set to render and index selection into.
    /// Everything downstream (operations, navigation, resolution) is board-
    /// agnostic and resolves the active board by id via `active_board`.
    pub fn displayed_boards(&self) -> &[Board] {
        if self.mode == AppMode::ArchivedBoardsView {
            self.model.archived_boards_flat()
        } else {
            self.model.boards()
        }
    }

    pub fn prepare_frame(&mut self) {
        match self.ctx.snapshot() {
            Ok(snapshot) => self.model.load_from_snapshot(snapshot),
            Err(e) => tracing::warn!("Failed to load snapshot for frame: {e}"),
        }

        // Cards are now one unified collection (live + archived); the view mode
        // selects which subset to display by filtering on `archived_card_ids`.
        // This inline filter is temporary: T1c introduces a single
        // `displayed_cards()` accessor that subsumes it (see KAN-914 D3 / KAN-931).
        let archived_ids = self.model.archived_card_ids();
        let want_archived = self.mode == AppMode::ArchivedCardsView;
        let cards_for_display: Vec<Card> = self
            .model
            .cards()
            .iter()
            .filter(|c| archived_ids.contains(&c.id) == want_archived)
            .cloned()
            .collect();
        let cards_for_display: &[Card] = &cards_for_display;

        // Board resolution: inlined (rather than calling `active_board` /
        // `displayed_boards`) because those return borrows tied to all of `self`,
        // which Rust NLL cannot split from the `&mut self.view.strategy` borrow
        // taken by `refresh_task_lists` below. Inlining keeps the borrow scoped to
        // `self.model`/`self.selection`/`self.mode`. When no board is active
        // (browsing the projects list), fall back to the highlighted board in the
        // currently displayed set so the tasks preview tracks the cursor.
        let displayed: &[Board] = if self.mode == AppMode::ArchivedBoardsView {
            self.model.archived_boards_flat()
        } else {
            self.model.boards()
        };
        let board: Option<&Board> = match self.selection.active_board_id {
            Some(id) => self.model.board_by_id(id),
            None => self
                .selection
                .board
                .get()
                .and_then(|idx| displayed.get(idx)),
        };

        if let Some(board) = board {
            let search_query = if self.filter.search.is_active {
                Some(self.filter.search.query())
            } else {
                None
            };
            let ctx = ViewRefreshContext {
                board,
                all_cards: cards_for_display,
                all_columns: self.model.columns(),
                all_sprints: self.model.sprints(),
                active_sprint_filters: self.filter.active_sprint_filters.clone(),
                hide_assigned_cards: self.filter.hide_assigned_cards,
                search_query,
            };
            self.view.strategy.refresh_task_lists(&ctx);
        }
        self.sync_card_list_component();
    }

    /// Undo the last action
    pub fn undo(&mut self) -> KanbanResult<()> {
        if self.ctx.undo()? {
            self.needs_redraw = true;
        } else {
            self.set_error("Nothing to undo".to_string());
        }
        Ok(())
    }

    /// Redo the last undone action
    pub fn redo(&mut self) -> KanbanResult<()> {
        if self.ctx.redo()? {
            self.needs_redraw = true;
        } else {
            self.set_error("Nothing to redo".to_string());
        }
        Ok(())
    }

    pub fn sync_card_list_component(&mut self) {
        if let Some(active_list) = self.view.strategy.get_active_task_list() {
            self.view
                .card_list_component
                .update_cards(active_list.cards.clone());
        }
    }

    pub fn switch_view_strategy(&mut self, task_list_view: kanban_domain::TaskListView) {
        let new_strategy: Box<dyn ViewStrategy> = match task_list_view {
            kanban_domain::TaskListView::Flat => Box::new(UnifiedViewStrategy::flat()),
            kanban_domain::TaskListView::GroupedByColumn => {
                Box::new(UnifiedViewStrategy::grouped())
            }
            kanban_domain::TaskListView::ColumnView => Box::new(UnifiedViewStrategy::kanban()),
        };

        self.view.strategy = new_strategy;
    }
}
