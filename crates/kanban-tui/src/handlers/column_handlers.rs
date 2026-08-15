use crate::app::{App, BoardFocus, DialogMode};
use crossterm::event::KeyCode;
use kanban_domain::card_lifecycle::sorted_board_columns;
use kanban_domain::commands::{
    BoardCommand, CardCommand, ColumnCommand, Command, CreateColumn, DeleteColumn, MoveCard,
    SetBoardTaskListView, UpdateColumn,
};
use kanban_domain::{ColumnUpdate, TaskListView};

impl App {
    pub fn handle_create_column_key(&mut self) {
        if self.focus.board_focus == BoardFocus::Columns {
            {
                if self.active_board().is_some() {
                    self.open_dialog(DialogMode::CreateColumn);
                    self.input.clear();
                }
            }
        }
    }

    pub fn handle_rename_column_key(&mut self) {
        if self.focus.board_focus == BoardFocus::Columns
            && self.dialog_input.column_list.get_selected_index().is_some()
        {
            {
                if let Some(board) = self.active_board() {
                    let board_id = board.id;
                    let board_columns = self.visible_board_columns(board_id);

                    if let Some(column_idx) = self.dialog_input.column_list.get_selected_index() {
                        if let Some(column) = board_columns.get(column_idx) {
                            self.input.set(column.name.clone());
                            self.open_dialog(DialogMode::RenameColumn);
                        }
                    }
                }
            }
        }
    }

    pub fn handle_set_column_default_status_key(&mut self) {
        if self.focus.board_focus == BoardFocus::Columns
            && self.dialog_input.column_list.get_selected_index().is_some()
        {
            if let Some(board) = self.active_board() {
                let board_id = board.id;
                if let Some(column_idx) = self.dialog_input.column_list.get_selected_index() {
                    if let Some(column) = self.visible_board_columns(board_id).get(column_idx) {
                        let idx = kanban_view::selection_dialog::popup_index_of_default_status(
                            column.default_status,
                        );
                        self.dialog_input.default_status_selection.set(Some(idx));
                        self.open_dialog(DialogMode::SetColumnDefaultStatus);
                    }
                }
            }
        }
    }

    pub fn handle_delete_column_key(&mut self) {
        if self.focus.board_focus == BoardFocus::Columns
            && self.dialog_input.column_list.get_selected_index().is_some()
        {
            {
                if let Some(board) = self.active_board() {
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
        if self.filter.column_search.is_active {
            return;
        }
        if self.focus.board_focus == BoardFocus::Columns
            && self.dialog_input.column_list.get_selected_index().is_some()
        {
            {
                if let Some(board) = self.active_board() {
                    let board_columns: Vec<(uuid::Uuid, i32)> =
                        sorted_board_columns(board.id, self.model.columns())
                            .into_iter()
                            .map(|col| (col.id, col.position))
                            .collect();

                    if let Some(selected_idx) = self.dialog_input.column_list.get_selected_index() {
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

                            self.dialog_input
                                .column_list
                                .update_item_count(board_columns.len());
                            self.dialog_input
                                .column_list
                                .set_selected_index(Some(selected_idx - 1));
                            tracing::info!("Moved column up");
                        }
                    }
                }
            }
        }
    }

    pub fn handle_move_column_down(&mut self) {
        if self.filter.column_search.is_active {
            return;
        }
        if self.focus.board_focus == BoardFocus::Columns
            && self.dialog_input.column_list.get_selected_index().is_some()
        {
            {
                if let Some(board) = self.active_board() {
                    let board_columns: Vec<(uuid::Uuid, i32)> =
                        sorted_board_columns(board.id, self.model.columns())
                            .into_iter()
                            .map(|col| (col.id, col.position))
                            .collect();

                    if let Some(selected_idx) = self.dialog_input.column_list.get_selected_index() {
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
                            self.dialog_input
                                .column_list
                                .update_item_count(column_count);
                            self.dialog_input
                                .column_list
                                .set_selected_index(Some(selected_idx + 1));
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

        if let Some(board) = self.active_board() {
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

    pub fn create_column(&mut self) {
        {
            // Collect board_id before command execution
            let board_id = self.active_board().map(|board| board.id);

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
                    default_status: None,
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
                    .column_list
                    .update_item_count(prior_column_count + 1);
                self.dialog_input
                    .column_list
                    .set_selected_index(Some(prior_column_count));
            }
        }
    }

    pub fn rename_column(&mut self) {
        {
            // Collect column ID before mutable borrow
            let column_info = {
                if let Some(board) = self.active_board() {
                    let board_id = board.id;
                    if let Some(column_idx) = self.dialog_input.column_list.get_selected_index() {
                        self.visible_board_columns(board_id)
                            .get(column_idx)
                            .map(|col| col.id)
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
        {
            // Collect all necessary data before mutating
            let delete_info = {
                if let Some(board) = self.active_board() {
                    let board_id = board.id;
                    if let Some(column_idx) = self.dialog_input.column_list.get_selected_index() {
                        let all_columns: Vec<(uuid::Uuid, String)> =
                            sorted_board_columns(board_id, self.model.columns())
                                .into_iter()
                                .map(|col| (col.id, col.name.clone()))
                                .collect();

                        if all_columns.len() <= 1 {
                            return;
                        }

                        // Resolved against the filtered list the confirm
                        // dialog was opened from, not `all_columns`, so a
                        // narrowed search doesn't delete the wrong column.
                        let column_to_delete = self
                            .visible_board_columns(board_id)
                            .get(column_idx)
                            .map(|col| (col.id, col.name.clone()));
                        let first_column_id = all_columns.first().map(|(id, _)| *id);

                        if let Some((column_id, column_name)) = column_to_delete {
                            let cards_to_move: Vec<(uuid::Uuid, i32)> = self
                                .model
                                .live_cards()
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
                    self.active_board()
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

                self.dialog_input
                    .column_list
                    .update_item_count(remaining_after_delete);
                if remaining_after_delete > 0 {
                    if column_idx >= remaining_after_delete {
                        self.dialog_input
                            .column_list
                            .set_selected_index(Some(remaining_after_delete - 1));
                    } else {
                        self.dialog_input
                            .column_list
                            .set_selected_index(Some(column_idx));
                    }
                } else {
                    self.dialog_input.column_list.set_selected_index(None);
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

                    if let Some(board_id) = self.active_board().map(|b| b.id) {
                        {
                            let cmd = Command::Board(BoardCommand::SetTaskListView(
                                SetBoardTaskListView { board_id, view },
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
    use crate::app::BoardFocus;
    use crate::App;
    use crossterm::event::KeyCode;

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
        // Column operations act on the active board (as when editing its detail).
        app.selection.active_board_id = app.model.boards().first().map(|b| b.id);
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

    #[test]
    fn test_move_column_up_swaps_correct_pair_regardless_of_model_iteration_order() {
        use kanban_domain::KanbanOperations;

        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");
        let board_id = app.ctx.data_store().list_boards().unwrap()[0].id;

        // Explicit-position create ties "New" with "Doing" (position 1);
        // "Doing" was created first, so canonical order is
        // [TODO(0), Doing(1), New(1), Complete(2)] -- Complete is last,
        // unambiguously, since its position (2) is unique.
        let doing_id = app
            .ctx
            .data_store()
            .list_columns_by_board(board_id)
            .unwrap()
            .iter()
            .find(|c| c.name == "Doing")
            .unwrap()
            .id;
        let complete_id = app
            .ctx
            .data_store()
            .list_columns_by_board(board_id)
            .unwrap()
            .iter()
            .find(|c| c.name == "Complete")
            .unwrap()
            .id;
        let new_col = app
            .ctx
            .create_column(board_id, "New".to_string(), Some(1))
            .unwrap();

        // Feed the model a snapshot with the tied pair's relative order
        // swapped from canonical, instead of going through the normal
        // ctx.snapshot() pipeline -- proving handle_move_column_up no longer
        // depends on the model happening to already be canonically ordered.
        let mut snapshot = app.ctx.snapshot().unwrap();
        let doing_idx = snapshot
            .columns
            .iter()
            .position(|c| c.id == doing_id)
            .unwrap();
        let new_idx = snapshot
            .columns
            .iter()
            .position(|c| c.id == new_col.id)
            .unwrap();
        snapshot.columns.swap(doing_idx, new_idx);
        app.model.load_from_snapshot(snapshot);
        app.selection.active_board_id = Some(board_id);

        // Complete is unambiguously last (index 3); moving it up must swap it
        // with "New" (its canonical predecessor, the later-created of the
        // tied pair) -- not "Doing", regardless of the scrambled model order.
        app.focus.board_focus = BoardFocus::Columns;
        app.dialog_input.column_list.update_item_count(4);
        app.dialog_input.column_list.set_selected_index(Some(3));
        app.handle_move_column_up();

        let doing = app.ctx.data_store().get_column(doing_id).unwrap().unwrap();
        let new = app
            .ctx
            .data_store()
            .get_column(new_col.id)
            .unwrap()
            .unwrap();
        let complete = app
            .ctx
            .data_store()
            .get_column(complete_id)
            .unwrap()
            .unwrap();

        assert_eq!(
            doing.position, 1,
            "Doing is not adjacent to Complete in canonical order and must be untouched"
        );
        assert_eq!(
            new.position, 2,
            "New (Complete's canonical predecessor) must be bumped to Complete's old position"
        );
        assert_eq!(
            complete.position, 1,
            "Complete must take New's old position"
        );
    }

    #[test]
    fn test_rename_column_resolves_correct_column_regardless_of_model_iteration_order() {
        use kanban_domain::KanbanOperations;

        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");
        let board_id = app.ctx.data_store().list_boards().unwrap()[0].id;

        let doing_id = app
            .ctx
            .data_store()
            .list_columns_by_board(board_id)
            .unwrap()
            .iter()
            .find(|c| c.name == "Doing")
            .unwrap()
            .id;
        let new_col = app
            .ctx
            .create_column(board_id, "New".to_string(), Some(1))
            .unwrap();

        let mut snapshot = app.ctx.snapshot().unwrap();
        let doing_idx = snapshot
            .columns
            .iter()
            .position(|c| c.id == doing_id)
            .unwrap();
        let new_idx = snapshot
            .columns
            .iter()
            .position(|c| c.id == new_col.id)
            .unwrap();
        snapshot.columns.swap(doing_idx, new_idx);
        app.model.load_from_snapshot(snapshot);
        app.selection.active_board_id = Some(board_id);

        // Canonical index 2 is "New" (Doing was created first). Selecting
        // index 2 and opening rename must populate "New"'s name, not
        // "Doing"'s, regardless of the scrambled model order.
        app.focus.board_focus = BoardFocus::Columns;
        app.dialog_input.column_list.update_item_count(4);
        app.dialog_input.column_list.set_selected_index(Some(2));
        app.handle_rename_column_key();

        assert_eq!(app.input.as_str(), "New");
    }

    #[test]
    fn test_rename_column_dialog_confirm_resolves_correct_column_regardless_of_model_iteration_order(
    ) {
        use kanban_domain::KanbanOperations;

        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");
        let board_id = app.ctx.data_store().list_boards().unwrap()[0].id;

        let doing_id = app
            .ctx
            .data_store()
            .list_columns_by_board(board_id)
            .unwrap()
            .iter()
            .find(|c| c.name == "Doing")
            .unwrap()
            .id;
        let new_col = app
            .ctx
            .create_column(board_id, "New".to_string(), Some(1))
            .unwrap();

        let mut snapshot = app.ctx.snapshot().unwrap();
        let doing_idx = snapshot
            .columns
            .iter()
            .position(|c| c.id == doing_id)
            .unwrap();
        let new_idx = snapshot
            .columns
            .iter()
            .position(|c| c.id == new_col.id)
            .unwrap();
        snapshot.columns.swap(doing_idx, new_idx);
        app.model.load_from_snapshot(snapshot);
        app.selection.active_board_id = Some(board_id);

        // Canonical index 2 is "New" (Doing was created first, tied at
        // position 1). Confirming a rename at index 2 must persist against
        // "New", not "Doing", regardless of the scrambled model order --
        // pins the same "resolve against the sorted/filtered list, not raw
        // model iteration order" contract, but for actual persistence
        // rather than just the dialog's pre-populated display text.
        app.focus.board_focus = BoardFocus::Columns;
        app.dialog_input.column_list.update_item_count(4);
        app.dialog_input.column_list.set_selected_index(Some(2));
        app.handle_rename_column_key();
        assert_eq!(app.input.as_str(), "New");

        app.input.set("Renamed".to_string());
        app.handle_rename_column_dialog(KeyCode::Enter);

        let doing = app.ctx.data_store().get_column(doing_id).unwrap().unwrap();
        let new_col_after = app
            .ctx
            .data_store()
            .get_column(new_col.id)
            .unwrap()
            .unwrap();
        assert_eq!(doing.name, "Doing", "Doing must be untouched");
        assert_eq!(
            new_col_after.name, "Renamed",
            "New (canonical index 2) must be the one actually renamed, regardless of scrambled model order"
        );
    }

    #[test]
    fn test_move_column_down_noop_while_column_search_active() {
        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");
        let board_id = app.ctx.data_store().list_boards().unwrap()[0].id;
        app.focus.board_focus = BoardFocus::Columns;
        app.dialog_input.column_list.update_item_count(3);
        app.dialog_input.column_list.set_selected_index(Some(0));
        let before: Vec<(uuid::Uuid, i32)> = app
            .ctx
            .data_store()
            .list_columns_by_board(board_id)
            .unwrap()
            .iter()
            .map(|c| (c.id, c.position))
            .collect();
        app.filter.column_search.activate();

        app.handle_move_column_down();

        let after: Vec<(uuid::Uuid, i32)> = app
            .ctx
            .data_store()
            .list_columns_by_board(board_id)
            .unwrap()
            .iter()
            .map(|c| (c.id, c.position))
            .collect();
        assert_eq!(
            before, after,
            "reorder must not touch positions while column search is active"
        );
    }

    #[test]
    fn test_move_column_up_noop_while_column_search_active() {
        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");
        let board_id = app.ctx.data_store().list_boards().unwrap()[0].id;
        app.focus.board_focus = BoardFocus::Columns;
        app.dialog_input.column_list.update_item_count(3);
        app.dialog_input.column_list.set_selected_index(Some(1));
        let before: Vec<(uuid::Uuid, i32)> = app
            .ctx
            .data_store()
            .list_columns_by_board(board_id)
            .unwrap()
            .iter()
            .map(|c| (c.id, c.position))
            .collect();
        app.filter.column_search.activate();

        app.handle_move_column_up();

        let after: Vec<(uuid::Uuid, i32)> = app
            .ctx
            .data_store()
            .list_columns_by_board(board_id)
            .unwrap()
            .iter()
            .map(|c| (c.id, c.position))
            .collect();
        assert_eq!(
            before, after,
            "reorder must not touch positions while column search is active"
        );
    }

    #[test]
    fn test_rename_column_dialog_confirm_renames_filtered_selection_not_unfiltered_index() {
        use kanban_domain::KanbanOperations;

        let mut app = App::test_default();
        let board = app.ctx.create_board("Board".into(), None).unwrap();
        for (name, position) in [
            ("Todo", 0),
            ("In Progress", 1),
            ("TODO Later", 2),
            ("Done", 3),
        ] {
            app.ctx
                .create_column(board.id, name.to_string(), Some(position))
                .unwrap();
        }
        let board_columns = app
            .ctx
            .data_store()
            .list_columns_by_board(board.id)
            .unwrap();
        let in_progress_id = board_columns
            .iter()
            .find(|c| c.name == "In Progress")
            .unwrap()
            .id;
        let todo_later_id = board_columns
            .iter()
            .find(|c| c.name == "TODO Later")
            .unwrap()
            .id;

        app.selection.active_board_id = Some(board.id);
        app.reload_model();
        app.prepare_frame();
        app.focus.board_focus = BoardFocus::Columns;

        // Filtered to [Todo, TODO Later] -- selecting filtered index 1 must
        // resolve to "TODO Later", not the unfiltered board's index 1
        // ("In Progress").
        app.filter.column_search.activate();
        for c in "todo".chars() {
            app.filter.column_search.input.insert_char(c);
        }
        app.dialog_input.column_list.update_item_count(2);
        app.dialog_input.column_list.set_selected_index(Some(1));

        app.handle_rename_column_key();
        assert_eq!(
            app.input.as_str(),
            "TODO Later",
            "the rename dialog must pre-populate with the filtered selection's name"
        );

        app.input.set("Renamed".to_string());
        app.handle_rename_column_dialog(KeyCode::Enter);

        let in_progress = app
            .ctx
            .data_store()
            .get_column(in_progress_id)
            .unwrap()
            .unwrap();
        let todo_later = app
            .ctx
            .data_store()
            .get_column(todo_later_id)
            .unwrap()
            .unwrap();

        assert_eq!(
            in_progress.name, "In Progress",
            "unfiltered index 1 (\"In Progress\") must not be touched by a filtered rename"
        );
        assert_eq!(
            todo_later.name, "Renamed",
            "the actually-selected filtered item (\"TODO Later\") must be the one renamed"
        );
    }
}
