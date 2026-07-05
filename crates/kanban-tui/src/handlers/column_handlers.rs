use crate::app::{App, BoardFocus, DialogMode};
use crossterm::event::KeyCode;
use kanban_domain::commands::{
    BoardCommand, CardCommand, ColumnCommand, Command, CreateColumn, DeleteColumn, MoveCard,
    SetBoardTaskListView, UpdateColumn,
};
use kanban_domain::{ColumnUpdate, TaskListView};

impl App {
    pub fn handle_create_column_key(&mut self) {
        if self.focus.board_focus == BoardFocus::Columns {
            if let Some(board_idx) = self.selection.board.get() {
                if self.model.boards().get(board_idx).is_some() {
                    self.open_dialog(DialogMode::CreateColumn);
                    self.input.clear();
                }
            }
        }
    }

    pub fn handle_rename_column_key(&mut self) {
        if self.focus.board_focus == BoardFocus::Columns
            && self.dialog_input.column_selection.get().is_some()
        {
            if let Some(board_idx) = self.selection.board.get() {
                let boards = self.model.boards();
                if let Some(board) = boards.get(board_idx) {
                    let columns = self.model.columns();
                    let board_columns: Vec<_> = columns
                        .iter()
                        .filter(|col| col.board_id == board.id)
                        .collect();

                    if let Some(column_idx) = self.dialog_input.column_selection.get() {
                        if let Some(column) = board_columns.get(column_idx) {
                            self.input.set(column.name.clone());
                            self.open_dialog(DialogMode::RenameColumn);
                        }
                    }
                }
            }
        }
    }

    pub fn handle_delete_column_key(&mut self) {
        if self.focus.board_focus == BoardFocus::Columns
            && self.dialog_input.column_selection.get().is_some()
        {
            if let Some(board_idx) = self.selection.board.get() {
                if let Some(board) = self.model.boards().get(board_idx) {
                    let column_count = self
                        .model
                        .columns()
                        .iter()
                        .filter(|col| col.board_id == board.id)
                        .count();

                    if column_count > 1 {
                        self.open_dialog(DialogMode::DeleteColumnConfirm);
                    } else {
                        tracing::warn!("Cannot delete the last column");
                    }
                }
            }
        }
    }

    pub fn handle_move_column_up(&mut self) {
        if self.focus.board_focus == BoardFocus::Columns
            && self.dialog_input.column_selection.get().is_some()
        {
            if let Some(board_idx) = self.selection.board.get() {
                if let Some(board) = self.model.boards().get(board_idx) {
                    // Collect and sort column data before mutating
                    let mut board_columns: Vec<_> = self
                        .model
                        .columns()
                        .iter()
                        .filter(|col| col.board_id == board.id)
                        .map(|col| (col.id, col.position))
                        .collect();

                    board_columns.sort_by_key(|(_, pos)| *pos);

                    if let Some(selected_idx) = self.dialog_input.column_selection.get() {
                        if selected_idx > 0 && selected_idx < board_columns.len() {
                            let prev_col_id = board_columns[selected_idx - 1].0;
                            let curr_col_id = board_columns[selected_idx].0;
                            let prev_pos = board_columns[selected_idx - 1].1;
                            let curr_pos = board_columns[selected_idx].1;

                            // Swap positions using batched commands
                            let cmd1 = Command::Column(ColumnCommand::Update(UpdateColumn {
                                column_id: prev_col_id,
                                updates: ColumnUpdate {
                                    position: Some(curr_pos),
                                    ..Default::default()
                                },
                            }));

                            let cmd2 = Command::Column(ColumnCommand::Update(UpdateColumn {
                                column_id: curr_col_id,
                                updates: ColumnUpdate {
                                    position: Some(prev_pos),
                                    ..Default::default()
                                },
                            }));

                            if let Err(e) = self.execute_commands_batch(vec![cmd1, cmd2]) {
                                tracing::error!("Failed to move column: {}", e);
                                self.set_error(format!("Failed to move column: {}", e));
                                return;
                            }

                            self.dialog_input.column_selection.prev();
                            tracing::info!("Moved column up");
                        }
                    }
                }
            }
        }
    }

    pub fn handle_move_column_down(&mut self) {
        if self.focus.board_focus == BoardFocus::Columns
            && self.dialog_input.column_selection.get().is_some()
        {
            if let Some(board_idx) = self.selection.board.get() {
                if let Some(board) = self.model.boards().get(board_idx) {
                    // Collect and sort column data before mutating
                    let mut board_columns: Vec<_> = self
                        .model
                        .columns()
                        .iter()
                        .filter(|col| col.board_id == board.id)
                        .map(|col| (col.id, col.position))
                        .collect();

                    board_columns.sort_by_key(|(_, pos)| *pos);

                    if let Some(selected_idx) = self.dialog_input.column_selection.get() {
                        if selected_idx < board_columns.len() - 1 {
                            let curr_col_id = board_columns[selected_idx].0;
                            let next_col_id = board_columns[selected_idx + 1].0;
                            let curr_pos = board_columns[selected_idx].1;
                            let next_pos = board_columns[selected_idx + 1].1;

                            // Swap positions using batched commands
                            let cmd1 = Command::Column(ColumnCommand::Update(UpdateColumn {
                                column_id: next_col_id,
                                updates: ColumnUpdate {
                                    position: Some(curr_pos),
                                    ..Default::default()
                                },
                            }));

                            let cmd2 = Command::Column(ColumnCommand::Update(UpdateColumn {
                                column_id: curr_col_id,
                                updates: ColumnUpdate {
                                    position: Some(next_pos),
                                    ..Default::default()
                                },
                            }));

                            if let Err(e) = self.execute_commands_batch(vec![cmd1, cmd2]) {
                                tracing::error!("Failed to move column: {}", e);
                                self.set_error(format!("Failed to move column: {}", e));
                                return;
                            }

                            let column_count = board_columns.len();
                            self.dialog_input.column_selection.next(column_count);
                            tracing::info!("Moved column down");
                        }
                    }
                }
            }
        }
    }

    pub fn handle_toggle_task_list_view(&mut self) {
        if self.focus.active != crate::app::Focus::Cards {
            return;
        }

        if let Some(board_idx) = self.selection.active_board_index {
            if let Some(board) = self.model.boards().get(board_idx) {
                let current_view_idx = match board.task_list_view {
                    TaskListView::Flat => 0,
                    TaskListView::GroupedByColumn => 1,
                    TaskListView::ColumnView => 2,
                };
                self.dialog_input
                    .task_list_view_selection
                    .set(Some(current_view_idx));
                self.open_dialog(DialogMode::SelectTaskListView);
            }
        }
    }

    pub fn create_column(&mut self) {
        if let Some(board_idx) = self.selection.board.get() {
            // Collect board_id before command execution
            let board_id = self.model.boards().get(board_idx).map(|board| board.id);

            if let Some(board_id) = board_id {
                let column_name = self.input.as_str().trim().to_string();

                if column_name.is_empty() {
                    tracing::warn!("Column name cannot be empty");
                    return;
                }

                let position = self
                    .model
                    .columns()
                    .iter()
                    .filter(|col| col.board_id == board_id)
                    .map(|col| col.position)
                    .max()
                    .unwrap_or(-1)
                    + 1;

                let cmd = Command::Column(ColumnCommand::Create(CreateColumn {
                    id: uuid::Uuid::new_v4(),
                    board_id,
                    name: column_name.clone(),
                    position,
                }));

                let prior_column_count = self
                    .model
                    .columns()
                    .iter()
                    .filter(|col| col.board_id == board_id)
                    .count();

                if let Err(e) = self.execute_command(cmd) {
                    tracing::error!("Failed to create column: {}", e);
                    self.set_error(format!("Failed to create column: {}", e));
                    return;
                }

                tracing::info!("Created column: {} (position: {})", column_name, position);

                self.dialog_input
                    .column_selection
                    .set(Some(prior_column_count));
            }
        }
    }

    pub fn rename_column(&mut self) {
        if let Some(board_idx) = self.selection.board.get() {
            // Collect column ID before mutable borrow
            let column_info = {
                let boards = self.model.boards();
                if let Some(board) = boards.get(board_idx) {
                    if let Some(column_idx) = self.dialog_input.column_selection.get() {
                        let columns = self.model.columns();
                        let board_columns: Vec<_> = columns
                            .iter()
                            .filter(|col| col.board_id == board.id)
                            .collect();

                        board_columns.get(column_idx).map(|col| col.id)
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some(column_id) = column_info {
                let new_name = self.input.as_str().trim().to_string();

                if new_name.is_empty() {
                    tracing::warn!("Column name cannot be empty");
                    return;
                }

                let cmd = Command::Column(ColumnCommand::Update(UpdateColumn {
                    column_id,
                    updates: ColumnUpdate {
                        name: Some(new_name.clone()),
                        ..Default::default()
                    },
                }));

                if let Err(e) = self.execute_command(cmd) {
                    tracing::error!("Failed to rename column: {}", e);
                    self.set_error(format!("Failed to rename column: {}", e));
                    return;
                }

                tracing::info!("Renamed column to: {}", new_name);
            }
        }
    }

    pub fn delete_column(&mut self) {
        if let Some(board_idx) = self.selection.board.get() {
            // Collect all necessary data before mutating
            let delete_info = {
                if let Some(board) = self.model.boards().get(board_idx) {
                    if let Some(column_idx) = self.dialog_input.column_selection.get() {
                        let board_columns: Vec<_> = self
                            .model
                            .columns()
                            .iter()
                            .filter(|col| col.board_id == board.id)
                            .map(|col| (col.id, col.name.clone()))
                            .collect();

                        if board_columns.len() <= 1 {
                            return;
                        }

                        let column_to_delete = board_columns.get(column_idx).cloned();
                        let first_column_id = board_columns.first().map(|(id, _)| *id);

                        if let Some((column_id, column_name)) = column_to_delete {
                            let cards_to_move: Vec<(uuid::Uuid, i32)> = self
                                .model
                                .cards()
                                .iter()
                                .filter(|card| card.column_id == column_id)
                                .map(|card| (card.id, card.position))
                                .collect();

                            Some((
                                column_id,
                                column_name,
                                first_column_id,
                                cards_to_move,
                                column_idx,
                            ))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some((column_id, column_name, first_column_id, cards_to_move, column_idx)) =
                delete_info
            {
                let remaining_after_delete = {
                    let columns = self.model.columns();
                    let board = self.model.boards().get(board_idx);
                    board
                        .map(|b| {
                            columns
                                .iter()
                                .filter(|c| c.board_id == b.id && c.id != column_id)
                                .count()
                        })
                        .unwrap_or(0)
                };

                tracing::warn!("Cannot delete the last column");

                // Build the full operation as one batch: move every
                // card to the first column, then delete the column.
                // One user action → one undo entry.
                let mut commands: Vec<Command> = Vec::new();
                if let Some(target_column_id) = first_column_id {
                    if target_column_id != column_id {
                        for (card_id, position) in cards_to_move {
                            commands.push(Command::Card(CardCommand::Move(MoveCard {
                                card_id,
                                new_column_id: target_column_id,
                                new_position: position,
                            })));
                        }
                    }
                }
                commands.push(Command::Column(ColumnCommand::Delete(DeleteColumn {
                    column_id,
                })));

                if let Err(e) = self.execute_commands_batch(commands) {
                    tracing::error!("Failed to delete column: {}", e);
                    self.set_error(format!("Failed to delete column: {}", e));
                    return;
                }

                tracing::info!("Deleted column: {}", column_name);

                if remaining_after_delete > 0 {
                    if column_idx >= remaining_after_delete {
                        self.dialog_input
                            .column_selection
                            .set(Some(remaining_after_delete - 1));
                    } else {
                        self.dialog_input.column_selection.set(Some(column_idx));
                    }
                } else {
                    self.dialog_input.column_selection.clear();
                }
            }
        }
    }

    pub fn handle_create_column_dialog(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Esc => {
                self.pop_mode();
                self.focus.board_focus = BoardFocus::Columns;
                self.input.clear();
            }
            KeyCode::Enter => {
                self.create_column();
                self.pop_mode();
                self.focus.board_focus = BoardFocus::Columns;
                self.input.clear();
            }
            KeyCode::Char(c) => {
                self.input.insert_char(c);
            }
            KeyCode::Backspace => {
                self.input.backspace();
            }
            KeyCode::Left => {
                self.input.move_left();
            }
            KeyCode::Right => {
                self.input.move_right();
            }
            _ => {}
        }
    }

    pub fn handle_rename_column_dialog(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Esc => {
                self.pop_mode();
                self.focus.board_focus = BoardFocus::Columns;
                self.input.clear();
            }
            KeyCode::Enter => {
                self.rename_column();
                self.pop_mode();
                self.focus.board_focus = BoardFocus::Columns;
                self.input.clear();
            }
            KeyCode::Char(c) => {
                self.input.insert_char(c);
            }
            KeyCode::Backspace => {
                self.input.backspace();
            }
            KeyCode::Left => {
                self.input.move_left();
            }
            KeyCode::Right => {
                self.input.move_right();
            }
            _ => {}
        }
    }

    pub fn handle_delete_column_confirm_popup(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.delete_column();
                self.pop_mode();
                self.focus.board_focus = BoardFocus::Columns;
            }
            KeyCode::Char('n')
            | KeyCode::Char('N')
            | KeyCode::Char('q')
            | KeyCode::Char('Q')
            | KeyCode::Esc => {
                self.pop_mode();
                self.focus.board_focus = BoardFocus::Columns;
            }
            _ => {}
        }
    }

    pub fn handle_select_task_list_view_popup(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Esc => {
                self.pop_mode();
                self.dialog_input.task_list_view_selection.clear();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.dialog_input.task_list_view_selection.next(3);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.dialog_input.task_list_view_selection.prev();
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(view_idx) = self.dialog_input.task_list_view_selection.get() {
                    let view = match view_idx {
                        0 => TaskListView::Flat,
                        1 => TaskListView::GroupedByColumn,
                        2 => TaskListView::ColumnView,
                        _ => TaskListView::Flat,
                    };

                    let selected_card_id = self.get_selected_card_id();

                    if let Some(board_idx) = self.selection.active_board_index {
                        if let Some(board) = self.model.boards().get(board_idx) {
                            let cmd = Command::Board(BoardCommand::SetTaskListView(
                                SetBoardTaskListView {
                                    board_id: board.id,
                                    view,
                                },
                            ));

                            if let Err(e) = self.execute_command(cmd) {
                                tracing::error!("Failed to set task list view: {}", e);
                                self.set_error(format!("Failed to set task list view: {}", e));
                                self.pop_mode();
                                self.dialog_input.task_list_view_selection.clear();
                                return;
                            }

                            self.switch_view_strategy(view);

                            if let Some(card_id) = selected_card_id {
                                self.select_card_by_id(card_id);
                            }

                            tracing::info!("Updated task list view to: {:?}", view);
                        }
                    }
                }
                self.pop_mode();
                self.dialog_input.task_list_view_selection.clear();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::App;

    /// Refresh the TUI model from the store so the create handlers (which read
    /// `self.model`) see prior writes. The event loop does this each frame via
    /// `prepare_frame`; tests pull the snapshot directly.
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

    fn create_named_column(app: &mut App, name: &str) {
        app.input.set(name.to_string());
        app.create_column();
        app.input.clear();
        refresh(app);
    }

    /// KAN-794: the TUI column-create entry point funnels through the Column
    /// factory (`Column::create` via the `CreateColumn` command), so a created
    /// column carries the factory's single-clock invariant (`created_at ==
    /// updated_at`) and appends after the three default columns the board seeds.
    #[test]
    fn test_tui_create_column_routes_through_factory() {
        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");
        let board_id = app.ctx.data_store().list_boards().unwrap()[0].id;

        create_named_column(&mut app, "In Review");

        let columns = app.ctx.data_store().list_all_columns().unwrap();
        let column = columns
            .iter()
            .find(|c| c.board_id == board_id && c.name == "In Review")
            .expect("created column present in store");
        // Factory uses one clock for both timestamps.
        assert_eq!(column.created_at, column.updated_at);
        // Appends after the three default columns (TODO/Doing/Complete).
        assert_eq!(column.position, 3);
    }

    /// The TUI create path rejects a blank/whitespace column name before any
    /// command is built, so no column is written.
    #[test]
    fn test_tui_create_column_rejects_blank_name() {
        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");
        let board_id = app.ctx.data_store().list_boards().unwrap()[0].id;
        let before = app
            .ctx
            .data_store()
            .list_all_columns()
            .unwrap()
            .iter()
            .filter(|c| c.board_id == board_id)
            .count();

        create_named_column(&mut app, "   ");

        let after = app
            .ctx
            .data_store()
            .list_all_columns()
            .unwrap()
            .iter()
            .filter(|c| c.board_id == board_id)
            .count();
        assert_eq!(after, before, "blank column name must not be written");
    }
}
