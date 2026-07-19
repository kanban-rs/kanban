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
        if self.focus.active == Focus::Boards && self.selection.board.get().is_some() {
            if let Some(board_idx) = self.selection.board.get() {
                if let Some(board) = self.model.boards().get(board_idx) {
                    self.input.set(board.name.clone());
                    self.open_dialog(DialogMode::RenameBoard);
                }
            }
        }
    }

    pub fn handle_edit_board_key(&mut self) {
        if self.focus.active == Focus::Boards && self.selection.board.get().is_some() {
            self.push_mode(AppMode::BoardDetail);
            self.focus.board_focus = BoardFocus::Name;
        }
    }

    pub fn handle_export_board_key(&mut self) {
        if self.focus.active == Focus::Boards && self.selection.board.get().is_some() {
            if let Some(board_idx) = self.selection.board.get() {
                if let Some(board) = self.model.boards().get(board_idx) {
                    let filename = format!(
                        "{}-{}.json",
                        board.name.replace(" ", "-").to_lowercase(),
                        chrono::Utc::now().format("%Y%m%d-%H%M%S")
                    );
                    self.input.set(filename);
                    self.open_dialog(DialogMode::ExportBoard);
                }
            }
        }
    }

    pub fn handle_export_all_key(&mut self) {
        if self.focus.active == Focus::Boards && !self.model.boards().is_empty() {
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
                if let Some(board_id) = self.model.boards().get(idx).map(|b| b.id) {
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

    /// ARCHIVE the highlighted board (the primary "remove from live" action,
    /// mirroring the card panel's `d`). Its subtree stays in place; the board head
    /// moves to the archived-boards view where it can be restored or permanently
    /// deleted. The selection bookkeeping below is identical to a live removal —
    /// the board simply leaves the live list.
    pub fn delete_board(&mut self) {
        let Some(idx) = self.selection.board.get() else {
            return;
        };
        let Some(board_id) = self.model.boards().get(idx).map(|b| b.id) else {
            return;
        };
        let remaining_after = self.model.boards().len().saturating_sub(1);

        // Capture the viewed board's layout BEFORE removing, while the model and
        // the active index are still valid. `None` when the viewed board is the
        // one being removed, or nothing is active. (Reading it AFTER would use the
        // stale model — `self.model` is only reloaded next frame — with the
        // post-shift index, yielding the wrong board's layout.)
        let surviving_view = match self.selection.active_board_index {
            Some(active) if active != idx => {
                self.model.boards().get(active).map(|b| b.task_list_view)
            }
            _ => None,
        };

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

        // Active/viewed board: keep pointing at the SAME board across the shift
        // caused by removing `idx`. The highlight and the active board are
        // independent (activate with Enter, then move the highlight with j/k),
        // so deriving `active` from the highlight would silently switch which
        // board is being viewed.
        self.selection.active_board_index = match self.selection.active_board_index {
            Some(active) if active == idx => None, // the viewed board itself was deleted
            Some(active) if active > idx => Some(active - 1), // elements after idx shift down
            other => other,                        // active < idx (unchanged) or None (stay None)
        };

        // View: apply the surviving board's captured layout. When no board is
        // viewed (deleted, or none active), reset to the default (Flat) strategy
        // whose active task list is an empty list, so the next
        // `sync_card_list_component` clears the cards panel rather than leaving
        // stale cards (a grouped/kanban strategy would expose no active list
        // and the sync would skip, leaving ghosts).
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
                self.prepare_frame();
                // Select the first archived board (if any).
                let has_any = !self.model.archived_boards_flat().is_empty();
                self.selection.board.set(has_any.then_some(0));
                self.needs_redraw = true;
            }
            AppMode::ArchivedBoardsView => {
                self.mode = AppMode::Normal;
                self.prepare_frame();
                let has_any = !self.model.boards().is_empty();
                self.selection.board.set(has_any.then_some(0));
                self.needs_redraw = true;
            }
            _ => {}
        }
    }

    /// The archived board currently highlighted in the ArchivedBoardsView.
    fn selected_archived_board_id(&self) -> Option<uuid::Uuid> {
        let idx = self.selection.board.get()?;
        self.model.archived_boards_flat().get(idx).map(|b| b.id)
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
        self.prepare_frame();
        // Clamp the highlight to the shrunken archived list.
        let remaining = self.model.archived_boards_flat().len();
        self.selection.board.set(
            (remaining > 0).then(|| self.selection.board.get().unwrap_or(0).min(remaining - 1)),
        );
        self.needs_redraw = true;
    }

    /// Permanently delete the highlighted archived board and its subtree (direct,
    /// mirroring the archived-cards permanent delete).
    pub fn handle_delete_archived_board(&mut self) {
        if self.mode != AppMode::ArchivedBoardsView {
            return;
        }
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
        let remaining = self.model.archived_boards_flat().len();
        self.selection.board.set(
            (remaining > 0).then(|| self.selection.board.get().unwrap_or(0).min(remaining - 1)),
        );
        self.needs_redraw = true;
    }

    pub fn create_board(&mut self) {
        let board_name = self.input.as_str().to_string();

        let board_id = uuid::Uuid::new_v4();
        let position = self.model.boards().len() as i32;
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
            if let Some(board) = self.model.boards().get(idx) {
                let board_id = board.id;
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
        app.selection
            .active_board_index
            .and_then(|i| app.model.boards().get(i))
            .map(|b| b.id)
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
    fn test_delete_non_active_board_keeps_viewed_board() {
        let mut app = App::test_default();
        create_named_board(&mut app, "A");
        create_named_board(&mut app, "B");
        create_named_board(&mut app, "C");
        create_named_board(&mut app, "D");
        let a_id = app.model.boards()[0].id;
        // Viewing A (index 0); highlight is on D (index 3).
        app.selection.active_board_index = Some(0);
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(3));

        app.delete_board();
        refresh(&mut app);

        assert_eq!(app.selection.active_board_index, Some(0));
        assert_eq!(
            active_board_id(&app),
            Some(a_id),
            "still viewing A, not switched"
        );
    }

    #[test]
    fn test_delete_board_before_active_shifts_active_index() {
        let mut app = App::test_default();
        create_named_board(&mut app, "A");
        create_named_board(&mut app, "B");
        create_named_board(&mut app, "C");
        let c_id = app.model.boards()[2].id;
        app.selection.active_board_index = Some(2); // viewing C
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(0)); // highlight A

        app.delete_board(); // delete A (index 0, before the active one)
        refresh(&mut app);

        assert_eq!(
            app.selection.active_board_index,
            Some(1),
            "active index shifts down by one"
        );
        assert_eq!(active_board_id(&app), Some(c_id), "still viewing C");
    }

    #[test]
    fn test_delete_active_board_clears_active() {
        let mut app = App::test_default();
        create_named_board(&mut app, "A");
        create_named_board(&mut app, "B");
        app.selection.active_board_index = Some(1); // viewing B
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(1)); // highlight B (the active board)

        app.delete_board();

        assert_eq!(
            app.selection.active_board_index, None,
            "active cleared when the viewed board is deleted"
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
        app.selection.active_board_index = Some(0); // viewing A
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
        // Regression: the viewed board sits AFTER the deleted one, so its index
        // shifts. The view must reflect the VIEWED board's layout, captured
        // before the delete, not a re-lookup by the shifted index into the
        // (stale, pre-delete) model.
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
        app.selection.active_board_index = Some(2); // viewing C
        app.focus.active = Focus::Boards;
        app.selection.board.set(Some(0)); // highlight A (before C)

        app.delete_board(); // delete A; C survives, its index shifts 2 -> 1

        assert_eq!(app.selection.active_board_index, Some(1));
        assert!(
            is_kanban_strategy(&app),
            "still shows C's ColumnView layout, not B's/the deleted board's"
        );
    }

    #[test]
    fn test_delete_last_board_leaves_no_cards() {
        let mut app = App::test_default();
        create_named_board(&mut app, "Solo");
        app.selection.active_board_index = Some(0);
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
}
