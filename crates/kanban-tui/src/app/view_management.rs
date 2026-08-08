use super::{App, AppMode};
use crate::view_strategy::UnifiedViewStrategy;
use kanban_domain::{Board, Card, KanbanResult};
use kanban_view::view_strategy::{ViewRefreshContext, ViewStrategy};
use uuid::Uuid;

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
        let id = self
            .selection
            .active_board_id
            .or_else(|| self.board_list.get_selected_board_id())?;
        self.model.board_by_id(id)
    }

    /// The card subset the tasks panel currently displays: the archived cards
    /// under the archived-cards view (stack-aware, so a confirm dialog opened
    /// over it still resolves the archived set), the live cards otherwise. A
    /// borrow of the partition cached on `load_from_snapshot` — no per-frame
    /// filter or clone. The SOLE card-side live/archived selector.
    pub fn displayed_cards(&self) -> &[Card] {
        let want_archived = matches!(self.get_base_mode(), AppMode::ArchivedCardsView);
        self.model.displayed_cards(want_archived)
    }

    /// The board set the projects panel currently displays: the archived heads
    /// when the panel is toggled to the archived set, the live boards otherwise.
    /// This is the SOLE place the live/archived distinction affects behavior —
    /// the projects panel choosing which set to render and index selection into.
    /// Everything downstream (operations, navigation, resolution) is board-
    /// agnostic and resolves the active board by id via `active_board`.
    ///
    /// Stack-aware: the choice keys off `get_base_mode()`, not the raw `mode`, so
    /// a confirm dialog opened over the archived view (mode == Dialog(..)) still
    /// resolves the archived heads — the underlay-bug fix. Returns a BORROW of
    /// the partition cached on `load_from_snapshot`, eliminating the per-redraw
    /// clone the interim owned-Vec form carried.
    pub fn displayed_boards(&self) -> &[Board] {
        let want_archived = matches!(self.get_base_mode(), AppMode::ArchivedBoardsView);
        self.model.displayed_boards(want_archived)
    }

    pub fn prepare_frame(&mut self) {
        match self.ctx.snapshot() {
            Ok(snapshot) => self.model.load_from_snapshot(snapshot),
            Err(e) => tracing::warn!("Failed to load snapshot for frame: {e}"),
        }

        // Single card-side selector: borrow the cached displayed subset (stack-
        // aware base mode). No per-frame filter/clone — the partition was built
        // on load. Resolved via `self.model` directly (not `self.displayed_cards`)
        // so the borrow is scoped to `self.model` and splits cleanly from the
        // `&mut self.view.strategy` borrow `refresh_task_lists` takes below.
        let want_archived_cards = matches!(self.get_base_mode(), AppMode::ArchivedCardsView);
        let cards_for_display: &[Card] = self.model.displayed_cards(want_archived_cards);

        // Board resolution: resolved via `self.model` directly (rather than
        // `active_board` / `displayed_boards`, which borrow all of `self`) so the
        // borrow stays scoped to `self.model` and NLL can split it from the
        // `&mut self.view.strategy` borrow `refresh_task_lists` needs. When no
        // board is active (browsing the projects list), fall back to the
        // highlighted board in the currently displayed set — the same cached,
        // base-mode-selected subset the projects panel indexes into — so the
        // tasks preview tracks the cursor. The id is extracted and re-resolved by
        // id so the returned borrow is tied to `self.model`, not a temporary.
        let want_archived_boards = matches!(self.get_base_mode(), AppMode::ArchivedBoardsView);
        let board_ids: Vec<Uuid> = self
            .model
            .displayed_boards(want_archived_boards)
            .iter()
            .map(|b| b.id)
            .collect();
        self.board_list.update_boards(board_ids);
        let highlighted_id: Option<Uuid> = self.board_list.get_selected_board_id();
        let board_id: Option<Uuid> = self.selection.active_board_id.or(highlighted_id);
        let board: Option<&Board> = board_id.and_then(|id| self.model.board_by_id(id));

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
