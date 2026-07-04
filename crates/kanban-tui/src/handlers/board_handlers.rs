use crate::app::{App, AppMode, BoardFocus, DialogMode, Focus};
use crossterm::event::KeyCode;
use kanban_domain::commands::{
    BoardCommand, ColumnCommand, Command, CreateBoard, CreateColumn, UpdateBoard,
};
use kanban_domain::{BoardUpdate, KanbanOperations, TaskListView};

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
                if self.model.boards().get(idx).is_some() {
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
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => self.pop_mode(),
            _ => {}
        }
    }

    pub fn delete_board(&mut self) {
        let Some(idx) = self.selection.board.get() else {
            return;
        };
        let Some(board_id) = self.model.boards().get(idx).map(|b| b.id) else {
            return;
        };
        let remaining_after = self.model.boards().len().saturating_sub(1);

        if let Err(e) = self.ctx.delete_board(board_id) {
            tracing::error!("Failed to delete board: {}", e);
            self.set_error(format!("Failed to delete board: {}", e));
            return;
        }
        tracing::info!("Deleted board {}", board_id);

        // Board-selection fixup: clamp to the surviving range or clear.
        if remaining_after == 0 {
            self.selection.board.clear();
            self.selection.active_board_index = None;
        } else {
            let new_idx = idx.min(remaining_after - 1);
            self.selection.board.set(Some(new_idx));
            self.selection.active_board_index = Some(new_idx);
        }
        self.switch_view_strategy(TaskListView::default());
    }

    /// (columns, live cards, archived cards, sprints) owned by `board_id`.
    /// Archived cards are scoped via `original_column_id` -> column -> board
    /// (pre-SE-B `ArchivedCard` carries no `board_id`).
    pub(crate) fn board_delete_counts(&self, board_id: uuid::Uuid) -> (usize, usize, usize, usize) {
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
            .filter(|a| col_ids.contains(&a.original_column_id))
            .count();
        let sprints = self
            .model
            .sprints()
            .iter()
            .filter(|s| s.board_id == board_id)
            .count();
        (columns, cards, archived, sprints)
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
    use crate::app::{AppMode, DialogMode, Focus};
    use crate::App;
    use crossterm::event::KeyCode;
    use kanban_domain::{CreateCardOptions, KanbanOperations};

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
    fn test_press_capital_d_on_boards_opens_delete_board_confirm() {
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
    fn test_delete_board_with_entities_removes_all() {
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

        assert!(
            app.ctx
                .data_store()
                .list_boards()
                .unwrap()
                .iter()
                .all(|b| b.id != board_id),
            "board gone"
        );
        assert!(
            app.ctx
                .data_store()
                .list_all_columns()
                .unwrap()
                .iter()
                .all(|c| c.board_id != board_id),
            "columns gone"
        );
        assert!(
            app.ctx
                .data_store()
                .list_all_cards()
                .unwrap()
                .iter()
                .all(|c| c.id != card.id),
            "card gone"
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
        assert_eq!(app.board_delete_counts(board_id), (3, 1, 0, 1));
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
}
