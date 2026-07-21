use crate::app::{App, AppMode, BoardFocus, DialogMode, Focus};
use crossterm::event::KeyCode;
use kanban_domain::commands::{
    BoardCommand, ColumnCommand, Command, CreateBoard, CreateColumn, UpdateBoard,
};
use kanban_domain::{BoardUpdate, KanbanOperations, TaskListView};

/// Entity counts owned by a board, shown in the delete-confirmation dialog.
/// Snapshotted once when the dialog opens so the modal never re-scans the
/// model per frame (and never proxies to a remote backend per tick).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BoardDeleteCounts {
    pub columns: usize,
    pub cards: usize,
    pub archived: usize,
    pub sprints: usize,
}

impl BoardDeleteCounts {
    pub(crate) fn is_empty(&self) -> bool {
        self.columns == 0 && self.cards == 0 && self.archived == 0 && self.sprints == 0
    }
}

impl App {
    pub fn handle_create_board_key(&mut self) {
        if self.focus.active == Focus::Boards {
            self.open_dialog(DialogMode::CreateBoard);
            self.input.clear();
        }
    }

    pub fn handle_rename_board_key(&mut self) {
        if self.focus.active == Focus::Boards {
            if let Some(name) = self
                .selection
                .board
                .get()
                .and_then(|idx| self.displayed_boards().get(idx).map(|b| b.name.clone()))
            {
                self.input.set(name);
                self.open_dialog(DialogMode::RenameBoard);
            }
        }
    }

    pub fn handle_edit_board_key(&mut self) {
        if self.focus.active == Focus::Boards {
            // Opening a board's detail makes it THE active board (by id), from
            // the currently displayed set — live or archived alike. Every detail
            // view then resolves it archival-agnostically via `active_board`.
            if let Some(board_id) = self
                .selection
                .board
                .get()
                .and_then(|idx| self.displayed_boards().get(idx).map(|b| b.id))
            {
                self.selection.active_board_id = Some(board_id);
                self.push_mode(AppMode::BoardDetail);
                self.focus.board_focus = BoardFocus::Name;
            }
        }
    }

    pub fn handle_export_board_key(&mut self) {
        if self.focus.active == Focus::Boards && self.selection.board.get().is_some() {
            if let Some(board_idx) = self.selection.board.get() {
                if let Some(board_name) = self
                    .displayed_boards()
                    .get(board_idx)
                    .map(|b| b.name.clone())
                {
                    let filename = format!(
                        "{}-{}.json",
                        board_name.replace(" ", "-").to_lowercase(),
                        chrono::Utc::now().format("%Y%m%d-%H%M%S")
                    );
                    self.input.set(filename);
                    self.open_dialog(DialogMode::ExportBoard);
                }
            }
        }
    }

    pub fn handle_export_all_key(&mut self) {
        if self.focus.active == Focus::Boards && self.model.live_boards().next().is_some() {
            let filename = format!(
                "kanban-all-{}.json",
                chrono::Utc::now().format("%Y%m%d-%H%M%S")
            );
            self.input.set(filename);
            self.open_dialog(DialogMode::ExportAll);
        }
    }

    pub fn handle_import_board_key(&mut self) {
        if self.focus.active == Focus::Boards {
            self.scan_import_files();
            if !self.dialog_input.import_files.is_empty() {
                self.dialog_input.import_selection.set(Some(0));
                self.open_dialog(DialogMode::ImportBoard);
            }
        }
    }

    pub fn handle_delete_board_key(&mut self) {
        if self.focus.active == Focus::Boards {
            if let Some(idx) = self.selection.board.get() {
                if let Some(board_id) = self.displayed_boards().get(idx).map(|b| b.id) {
                    // Snapshot the counts once, here, rather than re-scanning the
                    // model on every frame the modal is open.
                    self.dialog_input.board_delete_counts =
                        Some(self.board_delete_counts(board_id));
                    self.open_dialog(DialogMode::DeleteBoardConfirm);
                }
            }
        }
    }

    pub fn handle_delete_board_confirm_popup(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.delete_board();
                self.pop_mode();
                self.dialog_input.board_delete_counts = None;
            }
            KeyCode::Char('n')
            | KeyCode::Char('N')
            | KeyCode::Char('q')
            | KeyCode::Char('Q')
            | KeyCode::Esc => {
                self.pop_mode();
                self.dialog_input.board_delete_counts = None;
            }
            _ => {}
        }
    }

    /// Open the `DeletePermanentBoardConfirm` dialog for the highlighted archived
    /// board. Called when `x` is pressed in ArchivedBoardsView.
    pub fn handle_delete_archived_board_key(&mut self) {
        if self.mode != AppMode::ArchivedBoardsView {
            return;
        }
        let Some(board_id) = self.selected_archived_board_id() else {
            return;
        };
        self.dialog_input.board_delete_counts = Some(self.board_delete_counts(board_id));
        self.open_dialog(DialogMode::DeletePermanentBoardConfirm);
    }

    /// Handle a key press inside the `DeletePermanentBoardConfirm` dialog.
    pub fn handle_delete_permanent_board_confirm_popup(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.handle_delete_archived_board();
                self.pop_mode();
                self.dialog_input.board_delete_counts = None;
            }
            KeyCode::Char('n')
            | KeyCode::Char('N')
            | KeyCode::Char('q')
            | KeyCode::Char('Q')
            | KeyCode::Esc => {
                self.pop_mode();
                self.dialog_input.board_delete_counts = None;
            }
            _ => {}
        }
    }

    /// ARCHIVE the highlighted board (the primary "remove from live" action,
    /// mirroring the card panel's `d`). Its subtree stays in place; the board head
    /// moves to the archived-boards view where it can be restored or permanently
    /// deleted. The selection bookkeeping below is identical to a live removal —
    /// the board simply leaves the live list.
    pub fn delete_board(&mut self) {
        let Some(idx) = self.selection.board.get() else {
            return;
        };
        let Some(board_id) = self.displayed_boards().get(idx).map(|b| b.id) else {
            return;
        };
        let remaining_after = self.model.live_boards().count().saturating_sub(1);

        if let Err(e) = self.ctx.archive_board(board_id) {
            tracing::error!("Failed to archive board: {}", e);
            self.set_error(format!("Failed to archive board: {}", e));
            return;
        }
        tracing::info!("Archived board {}", board_id);

        // Highlight (selection.board): clamp to the surviving range, or clear.
        if remaining_after == 0 {
            self.selection.board.clear();
        } else {
            self.selection.board.set(Some(idx.min(remaining_after - 1)));
        }

        // Active/viewed board: tracked by IDENTITY, so it is naturally stable
        // across the list shift caused by removing a board — no index fixup. If
        // the board being archived is the one currently viewed, stop viewing it
        // (the projects list is the context now).
        if self.selection.active_board_id == Some(board_id) {
            self.selection.active_board_id = None;
        }

        // View: apply the still-active board's layout, or reset to the default
        // (Flat) strategy when nothing is viewed, so the next
        // `sync_card_list_component` clears the cards panel rather than leaving
        // stale cards (a grouped/kanban strategy would expose no active list and
        // the sync would skip, leaving ghosts). The model still holds the
        // archived head, so `active_board` resolves the surviving view.
        let surviving_view = self.active_board().map(|b| b.task_list_view);
        self.switch_view_strategy(surviving_view.unwrap_or_default());
    }

    /// Entity counts owned by `board_id` (columns, live cards, archived cards,
    /// sprints). Archived cards are scoped via the first-class `board_id` on the
    /// marker (survives a column deleted after archival).
    pub(crate) fn board_delete_counts(&self, board_id: uuid::Uuid) -> BoardDeleteCounts {
        let col_ids: std::collections::HashSet<uuid::Uuid> = self
            .model
            .columns()
            .iter()
            .filter(|c| c.board_id == board_id)
            .map(|c| c.id)
            .collect();
        let columns = col_ids.len();
        let cards = self
            .model
            .cards()
            .iter()
            .filter(|c| col_ids.contains(&c.column_id))
            .count();
        let archived = self
            .model
            .archived_cards()
            .iter()
            .filter(|a| a.context.board_id == board_id)
            .count();
        let sprints = self
            .model
            .sprints()
            .iter()
            .filter(|s| s.board_id == board_id)
            .count();
        BoardDeleteCounts {
            columns,
            cards,
            archived,
            sprints,
        }
    }

    /// Toggle between the live boards view and the archived-boards view (mirrors
    /// `handle_toggle_archived_cards_view`). Only meaningful when the Boards panel
    /// is the context; a no-op from unrelated modes.
    pub fn handle_toggle_archived_boards_view(&mut self) {
        match self.mode {
            AppMode::Normal if self.focus.active == Focus::Boards => {
                self.mode = AppMode::ArchivedBoardsView;
                // Toggling the displayed set returns to the projects list; any
                // board that was open is no longer active.
                self.selection.active_board_id = None;
                self.prepare_frame();
                // Select the first archived board (if any). In ArchivedBoardsView
                // `displayed_boards()` is the archived subset of the unified
                // collection.
                let has_any = !self.displayed_boards().is_empty();
                self.selection.board.set(has_any.then_some(0));
                self.needs_redraw = true;
            }
            AppMode::ArchivedBoardsView => {
                self.mode = AppMode::Normal;
                self.selection.active_board_id = None;
                self.prepare_frame();
                let has_any = self.model.live_boards().next().is_some();
                self.selection.board.set(has_any.then_some(0));
                self.needs_redraw = true;
            }
            _ => {}
        }
    }

    /// True when the projects panel is the active context and the board-sort
    /// affordances (order toggle / field picker) should fire: either the
    /// archived-boards view (stack-aware base mode, so it works under a pushed
    /// dialog) or the live projects panel with Boards focus.
    fn board_sort_context_active(&self) -> bool {
        matches!(self.get_base_mode(), AppMode::ArchivedBoardsView)
            || (matches!(self.get_base_mode(), AppMode::Normal)
                && self.focus.active == Focus::Boards)
    }

    /// Flip the board-list sort ORDER via the shared `SortOrder::toggled` (the
    /// SAME asc↔desc flip the task list's `handle_toggle_sort_order_key` applies)
    /// applied to the projects panel — LIVE and archived alike. The highlight
    /// tracks the same board across the re-sort so the cursor does not jump to a
    /// different project when the order changes, and the new field/order is saved
    /// to AppConfig so the choice survives a restart.
    ///
    /// Uses `get_base_mode()` (the stack-aware base), NOT the raw `mode`, so it
    /// fires even when a dialog is pushed over the archived view — matching how
    /// `displayed_boards`/render resolve which panel is showing.
    pub fn handle_toggle_board_sort_order(&mut self) {
        if !self.board_sort_context_active() {
            return;
        }
        let highlighted_id = self.highlighted_board_id();
        self.model.toggle_board_sort_order();
        self.repin_board_selection(highlighted_id);
        self.persist_board_sort();
        self.needs_redraw = true;
    }

    /// The archived board currently highlighted in the ArchivedBoardsView.
    /// Resolves against the archived subset of the unified collection directly
    /// (not `displayed_boards`, which is transiently the LIVE set while a confirm
    /// dialog is open on top of the archived view).
    fn selected_archived_board_id(&self) -> Option<uuid::Uuid> {
        let idx = self.selection.board.get()?;
        self.model.archived_boards_view().nth(idx).map(|b| b.id)
    }

    /// The board currently highlighted in the projects panel, resolved against
    /// the partition the panel is showing (archived under the archived view,
    /// live otherwise) via the stack-aware base mode.
    fn highlighted_board_id(&self) -> Option<uuid::Uuid> {
        let want_archived = matches!(self.get_base_mode(), AppMode::ArchivedBoardsView);
        let idx = self.selection.board.get()?;
        self.model
            .displayed_boards(want_archived)
            .get(idx)
            .map(|b| b.id)
    }

    /// Re-resolve the highlight to the same board's new index after a re-sort, so
    /// render and selection stay pinned to the same project; clamps to the first
    /// entry when the previously highlighted board is not present.
    fn repin_board_selection(&mut self, highlighted_id: Option<uuid::Uuid>) {
        let want_archived = matches!(self.get_base_mode(), AppMode::ArchivedBoardsView);
        let boards = self.model.displayed_boards(want_archived);
        let new_idx = highlighted_id.and_then(|id| boards.iter().position(|b| b.id == id));
        let count = boards.len();
        self.selection.board.set(match new_idx {
            Some(idx) => Some(idx),
            None => (count > 0).then_some(0),
        });
    }

    /// Persist the current board-list sort field/order to AppConfig via
    /// `kanban_service::config::save`, mirroring how the card toggle persists its
    /// sort (there via `SetTaskSort` onto the board; here onto the global config,
    /// since the projects-panel sort is a global UI preference, not per-board).
    fn persist_board_sort(&mut self) {
        use crate::app::model::{board_sort_field_to_config, board_sort_order_to_config};
        let (field, order) = self.model.board_sort();
        self.app_config.board_sort_field = Some(board_sort_field_to_config(field).to_string());
        self.app_config.board_sort_order = Some(board_sort_order_to_config(order).to_string());
        if let Err(e) = kanban_service::config::save(&self.app_config) {
            tracing::error!("Failed to persist board sort: {}", e);
            self.set_error(format!("Failed to persist board sort: {}", e));
        }
    }

    /// Open the board-sort field picker when the projects panel is the active
    /// context (live or archived). Primes the picker selection to the current
    /// field so it opens on the active row, mirroring `handle_order_cards_key`.
    pub fn handle_order_boards_key(&mut self) {
        if !self.board_sort_context_active() {
            return;
        }
        let sort_idx = self.get_current_board_sort_field_selection_index();
        self.filter.board_sort_field_selection.set(Some(sort_idx));
        self.open_dialog(DialogMode::OrderBoards);
    }

    /// Apply a board-sort field/order chosen from the picker: set the model sort,
    /// re-pin the highlight, and persist to AppConfig. Shared by the picker
    /// handler across live and archived.
    pub(crate) fn apply_board_sort(
        &mut self,
        field: kanban_domain::BoardSortField,
        order: kanban_domain::SortOrder,
    ) {
        let highlighted_id = self.highlighted_board_id();
        self.model.set_board_sort(field, order);
        self.repin_board_selection(highlighted_id);
        self.persist_board_sort();
        self.needs_redraw = true;
    }

    /// Restore the highlighted archived board back into the live set (direct,
    /// mirroring the archived-cards restore; no animation/multi-select).
    pub fn handle_restore_board(&mut self) {
        if self.mode != AppMode::ArchivedBoardsView {
            return;
        }
        let Some(board_id) = self.selected_archived_board_id() else {
            return;
        };
        if let Err(e) = self.ctx.restore_board(board_id) {
            tracing::error!("Failed to restore board: {}", e);
            self.set_error(format!("Failed to restore board: {}", e));
            return;
        }
        tracing::info!("Restored board {}", board_id);
        // If the restored board was the active one, it remains active — its id is
        // unchanged, `board_by_id` now finds it in the live set. Clear only if it
        // was the board being viewed and we want to drop back to the list; here
        // we keep the list context (restore is a list-view action).
        if self.selection.active_board_id == Some(board_id) {
            self.selection.active_board_id = None;
        }
        self.prepare_frame();
        // Clamp the highlight to the shrunken archived list.
        let remaining = self.model.archived_boards_view().count();
        self.selection.board.set(
            (remaining > 0).then(|| self.selection.board.get().unwrap_or(0).min(remaining - 1)),
        );
        self.needs_redraw = true;
    }

    /// Permanently delete the highlighted archived board and its subtree (direct,
    /// called after the `DeletePermanentBoardConfirm` dialog confirms).
    pub fn handle_delete_archived_board(&mut self) {
        let Some(board_id) = self.selected_archived_board_id() else {
            return;
        };
        if let Err(e) = self.ctx.delete_board(board_id) {
            tracing::error!("Failed to delete archived board: {}", e);
            self.set_error(format!("Failed to delete archived board: {}", e));
            return;
        }
        tracing::info!("Permanently deleted archived board {}", board_id);
        self.prepare_frame();
        let remaining = self.model.archived_boards_view().count();
        self.selection.board.set(
            (remaining > 0).then(|| self.selection.board.get().unwrap_or(0).min(remaining - 1)),
        );
        self.needs_redraw = true;
    }

    pub fn create_board(&mut self) {
        let board_name = self.input.as_str().to_string();

        let board_id = uuid::Uuid::new_v4();
        let position = self.model.live_boards().count() as i32;
        let new_index = position as usize;

        let mut commands: Vec<Command> = vec![Command::Board(BoardCommand::Create(CreateBoard {
            id: board_id,
            name: board_name.clone(),
            card_prefix: None,
            position,
        }))];

        for (name, position) in [("TODO", 0i32), ("Doing", 1i32), ("Complete", 2i32)] {
            commands.push(Command::Column(ColumnCommand::Create(CreateColumn {
                id: uuid::Uuid::new_v4(),
                board_id,
                name: name.to_string(),
                position,
            })));
        }

        // Single batch so undo reverses the whole "create a board"
        // action in one step.
        if let Err(e) = self.execute_commands_batch(commands) {
            tracing::error!("Failed to create board: {}", e);
            self.set_error(format!("Failed to create board: {}", e));
            return;
        }

        tracing::info!("Created board: {} (id: {})", board_name, board_id);

        self.selection.board.set(Some(new_index));
        self.switch_view_strategy(TaskListView::default());
    }

    pub fn rename_board(&mut self) {
        if let Some(idx) = self.selection.board.get() {
            if let Some(board_id) = self.displayed_boards().get(idx).map(|b| b.id) {
                let new_name = self.input.as_str().to_string();

                // Execute UpdateBoard command
                let cmd = Command::Board(BoardCommand::Update(UpdateBoard {
                    board_id,
                    updates: BoardUpdate {
                        name: Some(new_name.clone()),
                        ..Default::default()
                    },
                }));

                if let Err(e) = self.execute_command(cmd) {
                    tracing::error!("Failed to rename board: {}", e);
                    self.set_error(format!("Failed to rename board: {}", e));
                    return;
                }

                tracing::info!("Renamed board to: {}", new_name);
            }
        }
    }

    fn scan_import_files(&mut self) {
        self.dialog_input.import_files.clear();
        if let Ok(entries) = std::fs::read_dir(".") {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        if let Some(filename) = entry.file_name().to_str() {
                            if filename.ends_with(".json") {
                                self.dialog_input.import_files.push(filename.to_string());
                            }
                        }
                    }
                }
            }
        }
        self.dialog_input.import_files.sort();
    }
}

#[cfg(test)]
mod tests {
    use super::BoardDeleteCounts;
    use crate::app::{AppMode, DialogMode, Focus};
    use crate::App;
    use crossterm::event::KeyCode;
    use kanban_domain::{BoardUpdate, CreateCardOptions, KanbanOperations, TaskListView};

    /// Pull the store snapshot into `app.model` so handlers that read
    /// `self.model` observe prior writes (the event loop does this per frame).
    fn refresh(app: &mut App) {
        let snap = app.ctx.snapshot().unwrap();
        app.model.load_from_snapshot(snap);
    }

    fn create_named_board(app: &mut App, name: &str) {
        app.input.set(name.to_string());
        app.create_board();
        app.input.clear();
        refresh(app);
    }

    fn first_column_id(app: &App, board_id: uuid::Uuid) -> uuid::Uuid {
        app.ctx
            .data_store()
            .list_all_columns()
            .unwrap()
            .into_iter()
            .find(|c| c.board_id == board_id)
            .expect("board has a column")
            .id
    }

    #[test]
    fn test_delete_board_key_on_boards_opens_delete_board_confirm() {
        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(0));

        app.handle_delete_board_key();

        assert_eq!(app.mode, AppMode::Dialog(DialogMode::DeleteBoardConfirm));
    }

    #[test]
    fn test_delete_empty_board_removes_board() {
        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");
        let board_id = app.ctx.data_store().list_boards().unwrap()[0].id;
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(0));
        app.open_dialog(DialogMode::DeleteBoardConfirm);

        app.handle_delete_board_confirm_popup(KeyCode::Enter);

        let boards = app.ctx.data_store().list_boards().unwrap();
        assert!(boards.iter().all(|b| b.id != board_id), "board removed");
        assert_eq!(app.mode, AppMode::Normal, "dialog closed");
    }

    #[test]
    fn test_delete_board_archives_and_preserves_subtree() {
        // `delete_board()` now ARCHIVES (soft): the board leaves the LIVE list but
        // its subtree stays in place (C3b), and it appears in the archived list.
        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");
        let board_id = app.ctx.data_store().list_boards().unwrap()[0].id;
        let column_id = first_column_id(&app, board_id);
        let card = app
            .ctx
            .create_card(
                board_id,
                column_id,
                "Task".into(),
                CreateCardOptions::default(),
            )
            .unwrap();
        refresh(&mut app);
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(0));

        app.delete_board();

        // Gone from the LIVE board list (raw backend list_boards filters archived).
        assert!(
            app.ctx
                .data_store()
                .list_boards()
                .unwrap()
                .iter()
                .all(|b| b.id != board_id),
            "board left the live list"
        );
        // Present in the archived collection.
        assert!(
            app.ctx
                .list_archived_boards()
                .unwrap()
                .iter()
                .any(|ab| ab.entity_id == board_id),
            "board is archived"
        );
        // Subtree preserved in place (archive is not a delete).
        assert!(
            app.ctx
                .data_store()
                .list_all_columns()
                .unwrap()
                .iter()
                .any(|c| c.board_id == board_id),
            "columns preserved"
        );
        assert!(
            app.ctx
                .data_store()
                .list_all_cards()
                .unwrap()
                .iter()
                .any(|c| c.id == card.id),
            "card preserved"
        );
    }

    #[test]
    fn test_board_delete_counts_reports_entities() {
        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");
        let board_id = app.ctx.data_store().list_boards().unwrap()[0].id;
        let column_id = first_column_id(&app, board_id);
        app.ctx
            .create_card(
                board_id,
                column_id,
                "Task".into(),
                CreateCardOptions::default(),
            )
            .unwrap();
        app.ctx.create_sprint(board_id, None, None).unwrap();
        refresh(&mut app);

        // 3 default columns, 1 live card, 0 archived, 1 sprint.
        assert_eq!(
            app.board_delete_counts(board_id),
            BoardDeleteCounts {
                columns: 3,
                cards: 1,
                archived: 0,
                sprints: 1,
            }
        );
    }

    #[test]
    fn test_delete_confirm_cancel_keeps_board() {
        for cancel_key in [KeyCode::Char('n'), KeyCode::Esc] {
            let mut app = App::test_default();
            create_named_board(&mut app, "Roadmap");
            let board_id = app.ctx.data_store().list_boards().unwrap()[0].id;
            app.focus.active = Focus::Boards;
            app.selection.board.set(Some(0));
            app.open_dialog(DialogMode::DeleteBoardConfirm);

            app.handle_delete_board_confirm_popup(cancel_key);

            assert!(
                app.ctx
                    .data_store()
                    .list_boards()
                    .unwrap()
                    .iter()
                    .any(|b| b.id == board_id),
                "board intact after {cancel_key:?}"
            );
            assert_eq!(app.mode, AppMode::Normal);
        }
    }

    #[test]
    fn test_delete_board_is_undoable() {
        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");
        let board_id = app.ctx.data_store().list_boards().unwrap()[0].id;
        let column_id = first_column_id(&app, board_id);
        let card = app
            .ctx
            .create_card(
                board_id,
                column_id,
                "Task".into(),
                CreateCardOptions::default(),
            )
            .unwrap();
        refresh(&mut app);
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(0));
        app.delete_board();

        app.undo().unwrap();

        assert!(
            app.ctx
                .data_store()
                .list_boards()
                .unwrap()
                .iter()
                .any(|b| b.id == board_id),
            "board restored"
        );
        assert!(
            app.ctx
                .data_store()
                .list_all_cards()
                .unwrap()
                .iter()
                .any(|c| c.id == card.id),
            "card restored"
        );
    }

    #[test]
    fn test_delete_board_selection_clamps_after_delete() {
        let mut app = App::test_default();
        create_named_board(&mut app, "A");
        create_named_board(&mut app, "B");
        create_named_board(&mut app, "C");
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(2));

        app.delete_board();
        refresh(&mut app);
        assert_eq!(
            app.selection.board.get(),
            Some(1),
            "clamped to last survivor"
        );

        // Delete the remaining two; selection must clear at zero.
        app.selection.board.set(Some(1));
        app.delete_board();
        refresh(&mut app);
        app.selection.board.set(Some(0));
        app.delete_board();
        refresh(&mut app);
        assert_eq!(app.selection.board.get(), None, "selection cleared at zero");
        assert!(app.ctx.data_store().list_boards().unwrap().is_empty());
    }

    /// KAN-792: the TUI board-create entry point funnels through the Board
    /// factory (`Board::create`), so a created board carries the factory-seeded
    /// server-managed counters rather than a hand-assembled command's defaults.
    #[test]
    fn test_tui_create_board_routes_through_factory_seeds_counters() {
        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");

        let boards = app.ctx.data_store().list_boards().unwrap();
        let board = boards
            .iter()
            .find(|b| b.name == "Roadmap")
            .expect("created board present in store");
        // Factory seeds these; a hand-built create must not diverge.
        assert_eq!(board.card_counter, 1);
        assert_eq!(board.next_sprint_number, 1);
        assert_eq!(board.position, 0);

        // The "create a board" action still seeds the three default columns in
        // the same undoable batch.
        let columns = app.ctx.data_store().list_all_columns().unwrap();
        let names: Vec<&str> = columns
            .iter()
            .filter(|c| c.board_id == board.id)
            .map(|c| c.name.as_str())
            .collect();
        assert!(names.contains(&"TODO"), "default columns seeded: {names:?}");
        assert_eq!(names.len(), 3);
    }

    /// The factory validates the name: a blank/whitespace board name is rejected
    /// (no board written), where the old hand-built `CreateBoard` command path
    /// would have happily persisted a blank board.
    #[test]
    fn test_tui_create_board_rejects_blank_name_via_factory() {
        let mut app = App::test_default();
        create_named_board(&mut app, "   ");

        assert!(
            app.ctx.data_store().list_boards().unwrap().is_empty(),
            "factory must reject a blank board name"
        );
    }

    // ---- review fixes: active-board fixup, view, q-cancel, counts snapshot ----

    fn active_board_id(app: &App) -> Option<uuid::Uuid> {
        app.active_board().map(|b| b.id)
    }

    fn is_kanban_strategy(app: &App) -> bool {
        use crate::layout_strategy::ColumnListsLayout;
        use crate::view_strategy::UnifiedViewStrategy;
        app.view
            .strategy
            .as_any()
            .downcast_ref::<UnifiedViewStrategy>()
            .map(|u| {
                u.get_layout_strategy()
                    .as_any()
                    .downcast_ref::<ColumnListsLayout>()
                    .is_some()
            })
            .unwrap_or(false)
    }

    #[test]
    fn test_delete_non_active_board_keeps_active_board() {
        let mut app = App::test_default();
        create_named_board(&mut app, "A");
        create_named_board(&mut app, "B");
        create_named_board(&mut app, "C");
        create_named_board(&mut app, "D");
        let a_id = app.model.boards()[0].id;
        // Viewing A; highlight is on D (index 3).
        app.selection.active_board_id = Some(a_id);
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(3));

        app.delete_board();
        refresh(&mut app);

        assert_eq!(app.selection.active_board_id, Some(a_id));
        assert_eq!(
            active_board_id(&app),
            Some(a_id),
            "still viewing A, not switched"
        );
    }

    #[test]
    fn test_delete_board_before_active_keeps_viewing_same_board() {
        // Tracking the active board by id makes it shift-invariant: archiving a
        // board earlier in the list does not disturb which board is viewed.
        let mut app = App::test_default();
        create_named_board(&mut app, "A");
        create_named_board(&mut app, "B");
        create_named_board(&mut app, "C");
        let c_id = app.model.boards()[2].id;
        app.selection.active_board_id = Some(c_id); // viewing C
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(0)); // highlight A

        app.delete_board(); // archive A (before the active one)
        refresh(&mut app);

        assert_eq!(
            app.selection.active_board_id,
            Some(c_id),
            "active board id is stable across the list shift"
        );
        assert_eq!(active_board_id(&app), Some(c_id), "still viewing C");
    }

    #[test]
    fn test_delete_active_board_clears_active() {
        let mut app = App::test_default();
        create_named_board(&mut app, "A");
        create_named_board(&mut app, "B");
        let b_id = app.model.boards()[1].id;
        app.selection.active_board_id = Some(b_id); // viewing B
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(1)); // highlight B (the active board)

        app.delete_board();

        assert_eq!(
            app.selection.active_board_id, None,
            "active cleared when the viewed board is archived"
        );
    }

    #[test]
    fn test_delete_board_preserves_surviving_board_view() {
        let mut app = App::test_default();
        create_named_board(&mut app, "A");
        create_named_board(&mut app, "B");
        let a_id = app.model.boards()[0].id;
        // A uses the kanban (ColumnView) layout, not the Flat default.
        app.ctx
            .update_board(
                a_id,
                BoardUpdate {
                    task_list_view: Some(TaskListView::ColumnView),
                    ..Default::default()
                },
            )
            .unwrap();
        refresh(&mut app);
        app.selection.active_board_id = Some(a_id); // viewing A
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(1)); // highlight B

        app.delete_board(); // delete B; A survives as the viewed board

        assert!(
            is_kanban_strategy(&app),
            "surviving board's ColumnView layout is applied, not the Flat default"
        );
    }

    #[test]
    fn test_delete_board_before_active_preserves_viewed_view() {
        // The viewed board sits AFTER the archived one. With id-based tracking
        // the active board is stable, and its ColumnView layout is preserved.
        let mut app = App::test_default();
        create_named_board(&mut app, "A");
        create_named_board(&mut app, "B");
        create_named_board(&mut app, "C");
        let c_id = app.model.boards()[2].id;
        // C uses the kanban (ColumnView) layout; A and B keep the Flat default.
        app.ctx
            .update_board(
                c_id,
                BoardUpdate {
                    task_list_view: Some(TaskListView::ColumnView),
                    ..Default::default()
                },
            )
            .unwrap();
        refresh(&mut app);
        app.selection.active_board_id = Some(c_id); // viewing C
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(0)); // highlight A (before C)

        app.delete_board(); // archive A; C survives, unchanged id

        assert_eq!(app.selection.active_board_id, Some(c_id));
        assert!(
            is_kanban_strategy(&app),
            "still shows C's ColumnView layout, not B's/the archived board's"
        );
    }

    #[test]
    fn test_delete_last_board_leaves_no_cards() {
        let mut app = App::test_default();
        create_named_board(&mut app, "Solo");
        app.selection.active_board_id = app.model.boards().first().map(|b| b.id);
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(0));
        // Simulate a populated cards panel.
        app.view
            .card_list_component
            .update_cards(vec![uuid::Uuid::new_v4()]);
        assert!(!app.view.card_list_component.is_empty());

        app.delete_board();
        app.sync_card_list_component();

        assert!(
            app.view.card_list_component.is_empty(),
            "no ghost cards after deleting the last board"
        );
    }

    #[test]
    fn test_q_in_delete_board_confirm_cancels_not_quits() {
        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");
        let board_id = app.model.boards()[0].id;
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(0));
        app.handle_delete_board_key();
        assert_eq!(app.mode, AppMode::Dialog(DialogMode::DeleteBoardConfirm));

        app.handle_delete_board_confirm_popup(KeyCode::Char('q'));

        assert_eq!(app.mode, AppMode::Normal, "q closed the confirm dialog");
        assert!(
            app.ctx
                .data_store()
                .list_boards()
                .unwrap()
                .iter()
                .any(|b| b.id == board_id),
            "board still exists (q cancelled, did not delete or quit)"
        );
    }

    #[test]
    fn test_delete_board_counts_snapshotted_on_open() {
        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");
        let board_id = app.model.boards()[0].id;
        let column_id = first_column_id(&app, board_id);
        app.ctx
            .create_card(
                board_id,
                column_id,
                "Task".into(),
                CreateCardOptions::default(),
            )
            .unwrap();
        refresh(&mut app);
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(0));

        assert_eq!(
            app.dialog_input.board_delete_counts, None,
            "no stash before the dialog opens"
        );
        app.handle_delete_board_key();
        assert_eq!(
            app.dialog_input.board_delete_counts,
            Some(BoardDeleteCounts {
                columns: 3,
                cards: 1,
                archived: 0,
                sprints: 0,
            }),
            "counts snapshotted once when the dialog opens"
        );

        app.handle_delete_board_confirm_popup(KeyCode::Esc);
        assert_eq!(
            app.dialog_input.board_delete_counts, None,
            "stash cleared on close"
        );
    }

    // KAN-891: archived-board drill-down tests

    fn seed_archived_board_with_cards(app: &mut App, name: &str) -> (uuid::Uuid, uuid::Uuid) {
        create_named_board(app, name);
        let board_id = app
            .ctx
            .data_store()
            .list_boards()
            .unwrap()
            .into_iter()
            .find(|b| b.name == name)
            .unwrap()
            .id;
        let col_id = first_column_id(app, board_id);
        app.ctx
            .create_card(
                board_id,
                col_id,
                "Card1".into(),
                CreateCardOptions::default(),
            )
            .unwrap();
        app.ctx
            .create_card(
                board_id,
                col_id,
                "Card2".into(),
                CreateCardOptions::default(),
            )
            .unwrap();
        app.ctx.archive_board(board_id).unwrap();
        refresh(app);
        (board_id, col_id)
    }

    /// Activate the highlighted archived board through the SAME handler a live
    /// board uses. Proof of reuse: no archival-specific entry point.
    fn open_archived_board(app: &mut App) {
        app.mode = AppMode::ArchivedBoardsView;
        app.prepare_frame();
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(0));
        app.handle_selection_activate();
    }

    #[test]
    fn test_open_archived_board_populates_its_own_tasks() {
        let mut app = App::test_default();
        create_named_board(&mut app, "Live");
        let (arch_board_id, _) = seed_archived_board_with_cards(&mut app, "Arch");

        open_archived_board(&mut app);

        assert_eq!(
            app.selection.active_board_id,
            Some(arch_board_id),
            "archived board is the active board, tracked by id like a live one"
        );
        assert_eq!(app.focus.active, Focus::Cards);
        let task_count = app
            .view
            .strategy
            .get_active_task_list()
            .map(|l| l.len())
            .unwrap_or(0);
        assert_eq!(
            task_count, 2,
            "task list must show archived board's 2 cards"
        );
        assert!(
            app.ctx
                .list_archived_boards()
                .unwrap()
                .iter()
                .any(|ab| ab.entity_id == arch_board_id),
            "board remains archived after being opened"
        );
    }

    #[test]
    fn test_open_archived_board_with_zero_live_boards_still_opens() {
        let mut app = App::test_default();
        let (arch_board_id, _) = seed_archived_board_with_cards(&mut app, "OnlyBoard");

        open_archived_board(&mut app);

        assert_eq!(app.selection.active_board_id, Some(arch_board_id));
        assert_eq!(app.focus.active, Focus::Cards);
        assert!(
            app.ctx
                .list_archived_boards()
                .unwrap()
                .iter()
                .any(|ab| ab.entity_id == arch_board_id),
            "board still archived"
        );
    }

    #[test]
    fn test_enter_card_in_archived_board_opens_detail() {
        let mut app = App::test_default();
        let (_, _) = seed_archived_board_with_cards(&mut app, "Arch");

        open_archived_board(&mut app);

        assert_eq!(app.focus.active, Focus::Cards);
        if let Some(list) = app.view.strategy.get_active_task_list_mut() {
            list.set_selected_index(Some(0));
        }

        // Same activation handler a live board's card uses — no archival branch.
        app.handle_selection_activate();

        assert_eq!(app.mode, AppMode::CardDetail);
        assert!(app.selection.active_card_id.is_some());
    }

    #[test]
    fn test_escape_from_archived_board_returns_to_archived_list() {
        let mut app = App::test_default();
        let (_, _) = seed_archived_board_with_cards(&mut app, "Arch");

        open_archived_board(&mut app);

        app.handle_escape_key();

        assert_eq!(
            app.selection.active_board_id, None,
            "leaving the board drops back to the projects list"
        );
        assert_eq!(app.focus.active, Focus::Boards);
        assert_eq!(
            app.mode,
            AppMode::ArchivedBoardsView,
            "panel still shows the archived set"
        );
    }

    #[test]
    fn test_restore_clears_active_board() {
        let mut app = App::test_default();
        let (_, _) = seed_archived_board_with_cards(&mut app, "Arch");

        open_archived_board(&mut app);
        assert!(app.selection.active_board_id.is_some());

        app.focus.active = Focus::Boards;
        app.handle_restore_board();

        assert_eq!(app.selection.active_board_id, None);
    }

    // KAN-935: extension keys + underlay-mode correctness

    /// Permanent-delete keeps a confirm dialog. Pressing `x` in the archived view
    /// opens `DeletePermanentBoardConfirm` (pushed over the archived view, so the
    /// base mode stays `ArchivedBoardsView` and the underlay still shows the
    /// archived set); confirming actually removes the board and its subtree.
    #[test]
    fn test_permanent_delete_confirm_in_archived_view() {
        let mut app = App::test_default();
        let (arch_board_id, _) = seed_archived_board_with_cards(&mut app, "Arch");
        app.mode = AppMode::ArchivedBoardsView;
        app.prepare_frame();
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(0));

        // `x` opens the confirm dialog rather than deleting immediately.
        app.handle_archived_boards_view_mode(KeyCode::Char('x'));
        assert_eq!(
            app.mode,
            AppMode::Dialog(DialogMode::DeletePermanentBoardConfirm),
            "x opens the permanent-delete confirm dialog"
        );
        // Underlay fix: base mode (what the projects panel renders) is still the
        // archived set while the modal is open, not the live set.
        assert_eq!(
            *app.get_base_mode(),
            AppMode::ArchivedBoardsView,
            "confirm dialog is pushed over the archived view (base mode preserved)"
        );
        assert!(
            app.ctx
                .list_archived_boards()
                .unwrap()
                .iter()
                .any(|ab| ab.entity_id == arch_board_id),
            "board still present until the confirm is accepted"
        );

        // Confirm removes the board entirely.
        app.handle_delete_permanent_board_confirm_popup(KeyCode::Enter);
        assert_eq!(app.mode, AppMode::ArchivedBoardsView, "dialog closed");
        assert!(
            app.ctx
                .list_archived_boards()
                .unwrap()
                .iter()
                .all(|ab| ab.entity_id != arch_board_id),
            "board permanently deleted after confirm"
        );
        assert!(
            app.ctx
                .data_store()
                .list_all_columns()
                .unwrap()
                .iter()
                .all(|c| c.board_id != arch_board_id),
            "board's subtree deleted with it"
        );
    }

    /// Cancelling the permanent-delete confirm keeps the archived board.
    #[test]
    fn test_permanent_delete_confirm_cancel_keeps_board() {
        let mut app = App::test_default();
        let (arch_board_id, _) = seed_archived_board_with_cards(&mut app, "Arch");
        app.mode = AppMode::ArchivedBoardsView;
        app.prepare_frame();
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(0));

        app.handle_archived_boards_view_mode(KeyCode::Char('x'));
        app.handle_delete_permanent_board_confirm_popup(KeyCode::Esc);

        assert_eq!(
            app.mode,
            AppMode::ArchivedBoardsView,
            "back to archived view"
        );
        assert!(
            app.ctx
                .list_archived_boards()
                .unwrap()
                .iter()
                .any(|ab| ab.entity_id == arch_board_id),
            "board intact after cancel"
        );
    }

    /// The LIVE projects panel excludes archived boards. `displayed_boards()` in
    /// Normal mode returns only the live set, so an archived board never leaks into
    /// the live list (the sole data-source distinction).
    #[test]
    fn test_live_projects_panel_excludes_archived_boards() {
        let mut app = App::test_default();
        create_named_board(&mut app, "Live");
        let (arch_board_id, _) = seed_archived_board_with_cards(&mut app, "Arch");

        app.mode = AppMode::Normal;
        app.focus.active = Focus::Boards;
        app.prepare_frame();

        let displayed: Vec<uuid::Uuid> = app.displayed_boards().iter().map(|b| b.id).collect();
        assert!(
            displayed.iter().all(|id| *id != arch_board_id),
            "archived board must not appear in the live projects panel"
        );
        assert!(
            app.displayed_boards().iter().any(|b| b.name == "Live"),
            "live board is still shown"
        );

        // Toggling to the archived view flips the set (and only the set).
        app.mode = AppMode::ArchivedBoardsView;
        app.prepare_frame();
        assert!(
            app.displayed_boards().iter().any(|b| b.id == arch_board_id),
            "archived board appears once the panel is toggled to the archived set"
        );
    }

    /// Drilling into an archived board reuses the SAME activation handler a live
    /// board uses (Enter/Space → `handle_selection_activate`) and, from the
    /// archived boards LIST, `S` opens settings through the SAME shared handler as
    /// the live list — proving no archival branching in the operation handlers.
    #[test]
    fn test_archived_board_drill_in_and_settings_use_shared_handlers() {
        let mut app = App::test_default();
        let (arch_board_id, _) = seed_archived_board_with_cards(&mut app, "Arch");

        // Drill-in: the archived board becomes THE active board via the shared
        // activation handler (no archival-specific entry point).
        open_archived_board(&mut app);
        assert_eq!(app.focus.active, Focus::Cards, "drilled into the board");
        assert_eq!(app.selection.active_board_id, Some(arch_board_id));

        // Back to the archived boards list; `S` dispatched through the archived
        // view's key handler opens settings via the SHARED handler, board-set-
        // agnostic (same as from the live projects panel) — proving the dispatch
        // delegates rather than intercepting.
        app.handle_escape_key();
        assert_eq!(app.focus.active, Focus::Boards);
        app.handle_archived_boards_view_mode(KeyCode::Char('S'));
        assert_eq!(
            app.mode,
            AppMode::Settings,
            "settings opens from the archived boards list like the live one"
        );
    }

    /// The archived-boards list defaults to recency (newest-archived first) and
    /// the shared SortOrder toggle (`s`) reverses it, with the rendered list and
    /// the selection resolver staying consistent (both read the sorted partition).
    #[test]
    fn test_archived_boards_view_defaults_to_recency_and_s_toggles_order() {
        let mut app = App::test_default();
        // Archived in sequence: Arch2 is archived AFTER Arch1, so it is newer.
        let (arch1, _) = seed_archived_board_with_cards(&mut app, "Arch1");
        let (arch2, _) = seed_archived_board_with_cards(&mut app, "Arch2");

        app.mode = AppMode::ArchivedBoardsView;
        app.prepare_frame();
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(0));

        // Default: recency DESC → newest (Arch2) first.
        let rendered: Vec<uuid::Uuid> = app.displayed_boards().iter().map(|b| b.id).collect();
        assert_eq!(
            rendered,
            vec![arch2, arch1],
            "default archived order is newest-archived first (recency)"
        );

        // Highlight the top row (Arch2), then toggle order via the shared 's'.
        app.selection.board.set(Some(0));
        app.handle_archived_boards_view_mode(KeyCode::Char('s'));

        // Reversed: oldest (Arch1) first.
        let rendered: Vec<uuid::Uuid> = app.displayed_boards().iter().map(|b| b.id).collect();
        assert_eq!(
            rendered,
            vec![arch1, arch2],
            "'s' reverses the archived-boards order via the shared SortOrder toggle"
        );

        // Render and selection stay consistent: the highlight followed Arch2 to
        // its NEW index (1), so the resolved id still matches the rendered row.
        let sel_idx = app.selection.board.get().unwrap();
        assert_eq!(sel_idx, 1, "highlight tracked Arch2 to its new position");
        assert_eq!(
            app.displayed_boards()[sel_idx].id,
            arch2,
            "the rendered row at the selected index is the same board the resolver returns"
        );
    }
}
