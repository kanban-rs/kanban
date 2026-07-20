use super::{App, AppMode};
use crate::view_strategy::{UnifiedViewStrategy, ViewRefreshContext, ViewStrategy};
use kanban_domain::{Board, Card, KanbanResult};

impl App {
    /// Resolve the board currently being viewed (drilled into) — either a live
    /// board (when `active_board_index` is set) or an archived board head (when
    /// `active_archived_board_index` is set). Returns `None` when no board is
    /// active (e.g. the user is browsing the boards list without opening one).
    ///
    /// Note: `prepare_frame` inlines the equivalent resolution instead of calling
    /// this helper because it also needs a mutable borrow of `self.view.strategy`
    /// in the same scope, which Rust's NLL cannot split through a method boundary.
    /// All other call sites use this helper directly.
    pub fn viewed_board(&self) -> Option<&Board> {
        if let Some(ai) = self.selection.active_archived_board_index {
            self.model.archived_boards_flat().get(ai)
        } else {
            self.selection
                .active_board_index
                .or(self.selection.board.get())
                .and_then(|idx| self.model.boards().get(idx))
        }
    }

    pub fn prepare_frame(&mut self) {
        match self.ctx.snapshot() {
            Ok(snapshot) => self.model.load_from_snapshot(snapshot),
            Err(e) => tracing::warn!("Failed to load snapshot for frame: {e}"),
        }

        let cards_for_display: &[Card] = if self.mode == AppMode::ArchivedCardsView {
            self.model.archived_cards_flat()
        } else {
            self.model.cards()
        };

        // Board resolution: inlined here because a call to `self.viewed_board()`
        // (returning `Option<&Board>`) would borrow `self` immutably while
        // `self.view.strategy.refresh_task_lists` below needs a mutable borrow
        // of `self.view.strategy`. Rust NLL cannot split these through a method
        // call boundary, so we inline the same logic.
        let board: Option<&Board> = if let Some(ai) = self.selection.active_archived_board_index {
            self.model.archived_boards_flat().get(ai)
        } else {
            self.selection
                .active_board_index
                .or(self.selection.board.get())
                .and_then(|idx| self.model.boards().get(idx))
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
