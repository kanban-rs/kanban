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
    pub fn handle_board_selection_toggle(&mut self) {
        if self.focus.active == Focus::Boards {
            if self.multi_select.board_selection_mode_active {
                self.multi_select.board_selection_mode_active = false;
            } else {
                self.multi_select.board_selection_mode_active = true;
                if let Some(board_id) = self.board_list.get_selected_board_id() {
                    self.multi_select.selected_boards.insert(board_id);
                }
            }
        }
    }

    pub fn handle_clear_board_selection(&mut self) {
        self.multi_select.selected_boards.clear();
    }

    pub fn handle_select_all_boards_in_view(&mut self) {
        if self.focus.active != Focus::Boards {
            return;
        }

        for &id in self.board_list.ids() {
            self.multi_select.selected_boards.insert(id);
        }
        if !self.board_list.is_empty() {
            self.multi_select.board_selection_mode_active = true;
        }
    }

    pub fn handle_create_board_key(&mut self) {
        if self.focus.active == Focus::Boards {
            self.open_dialog(DialogMode::CreateBoard);
            self.input.clear();
        }
    }

    pub fn handle_rename_board_key(&mut self) {
        if self.focus.active == Focus::Boards {
            if let Some(name) = self
                .board_list
                .get_selected_board_id()
                .and_then(|id| self.model.board_by_id(id))
                .map(|b| b.name.clone())
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
            if let Some(board_id) = self.board_list.get_selected_board_id() {
                self.selection.active_board_id = Some(board_id);
                self.push_mode(AppMode::BoardDetail);
                self.focus.board_focus = BoardFocus::Name;
            }
        }
    }

    pub fn handle_export_board_key(&mut self) {
        if self.focus.active == Focus::Boards {
            if let Some(board_name) = self
                .board_list
                .get_selected_board_id()
                .and_then(|id| self.model.board_by_id(id))
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
            if let Some(board_id) = self.board_list.get_selected_board_id() {
                // Snapshot the counts once, here, rather than re-scanning the
                // model on every frame the modal is open.
                self.dialog_input.board_delete_counts = Some(self.board_delete_counts(board_id));
                self.open_dialog(DialogMode::DeleteBoardConfirm);
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
        let Some(board_id) = self.board_list.get_selected_board_id() else {
            return;
        };
        let idx = self.board_list.get_selected_index().unwrap_or(0);
        let remaining_after = self.model.live_boards().count().saturating_sub(1);

        if let Err(e) = self.ctx.archive_board(board_id) {
            tracing::error!("Failed to archive board: {}", e);
            self.set_error(format!("Failed to archive board: {}", e));
            return;
        }
        tracing::info!("Archived board {}", board_id);
        self.reload_model();

        // Highlight: clamp to the surviving range, or clear. Set directly on
        // `board_list` (rather than relying on the next `prepare_frame` resync,
        // which preserves by IDENTITY) because archiving must land on the same
        // POSITION the removed board vacated, not jump to the first board.
        if remaining_after == 0 {
            self.board_list.inner_mut().set_selected_index(None);
        } else {
            self.board_list
                .inner_mut()
                .set_selected_index(Some(idx.min(remaining_after - 1)));
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
            .live_cards()
            .iter()
            .filter(|c| col_ids.contains(&c.column_id))
            .count();
        let archived = self
            .model
            .archived_card_markers()
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
                // `prepare_frame` resyncs `board_list` from the new (archived)
                // partition; the previously highlighted live board's id is not in
                // it, so `BoardList::update_boards` falls back to the first
                // archived board (or `None` if empty) on its own.
                self.prepare_frame();
                self.needs_redraw = true;
            }
            AppMode::ArchivedBoardsView => {
                self.mode = AppMode::Normal;
                self.selection.active_board_id = None;
                self.prepare_frame();
                self.needs_redraw = true;
            }
            _ => {}
        }
    }

    /// True when the projects panel is the active context and the board-sort
    /// affordances (order toggle / field picker) should fire: either the
    /// archived-boards view or the live projects panel with Boards focus. In both
    /// cases the user must be BROWSING the board list, not viewing a board's
    /// contents: once a board is activated (`active_board_id` set, focus moves to
    /// Cards) the mode can still be `ArchivedBoardsView`, but `s`/`o` must be
    /// inert there (KAN-955).
    fn board_sort_context_active(&self) -> bool {
        let browsing_boards =
            self.selection.active_board_id.is_none() && self.focus.active == Focus::Boards;
        browsing_boards && matches!(self.mode, AppMode::ArchivedBoardsView | AppMode::Normal)
    }

    /// Flip the active partition's sort ORDER via the shared `SortOrder::toggled`
    /// (the SAME asc↔desc flip the task list's `handle_toggle_sort_order_key`
    /// applies), independently for live vs archived. The highlight tracks the
    /// same board across the re-sort so the cursor does not jump to a different
    /// project when the order changes; only the live choice is saved to
    /// AppConfig so it survives a restart.
    ///
    /// The guard reads the RAW `mode`: this handler is only ever reached from the
    /// live/archived board providers, which the keybinding router selects on the
    /// raw mode, so a pushed dialog can never reach this handler. `get_base_mode`
    /// is still used below (rather than the already-equivalent raw `mode`) to stay
    /// consistent with `apply_board_sort`, which does need it.
    pub fn handle_toggle_board_sort_order(&mut self) {
        if !self.board_sort_context_active() {
            return;
        }
        let want_archived = matches!(self.get_base_mode(), AppMode::ArchivedBoardsView);
        self.model.toggle_board_sort_order(want_archived);
        self.repin_board_selection();
        if !want_archived {
            self.persist_board_sort();
        }
        self.needs_redraw = true;
    }

    /// The archived board currently highlighted in the ArchivedBoardsView.
    /// `board_list` is synced against the same archived partition
    /// `archived_boards_view` reads, so its selection resolves directly by id —
    /// no index re-lookup.
    fn selected_archived_board_id(&self) -> Option<uuid::Uuid> {
        self.board_list.get_selected_board_id()
    }

    /// Re-sync `board_list` from the current sort order and re-select the
    /// board it already had highlighted (by id), so render and selection stay
    /// pinned to the same project across a re-sort; falls back to the first
    /// entry when that board is no longer present in the resorted set.
    fn repin_board_selection(&mut self) {
        let want_archived = matches!(self.get_base_mode(), AppMode::ArchivedBoardsView);
        let ids: Vec<uuid::Uuid> = self
            .model
            .displayed_boards(want_archived)
            .iter()
            .map(|b| b.id)
            .collect();
        self.board_list.update_boards(ids);
    }

    /// Persist the LIVE board-list sort field/order to AppConfig via
    /// `kanban_service::config::save`, mirroring how the card toggle persists its
    /// sort (there via `SetTaskSort` onto the board; here onto the global config,
    /// since the projects-panel sort is a global UI preference, not per-board).
    /// Callers must only invoke this for the live context — the archived sort is
    /// session-only and never persisted.
    fn persist_board_sort(&mut self) {
        let (field, order) = self.model.board_sort(false);
        let mut config = self.app_config.clone();
        config.board_sort_field = Some(field.to_string());
        config.board_sort_order = Some(order.to_string());
        self.set_app_config(config);
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
        let want_archived = matches!(self.get_base_mode(), AppMode::ArchivedBoardsView);
        self.model.set_board_sort(want_archived, field, order);
        self.repin_board_selection();
        if !want_archived {
            self.persist_board_sort();
        }
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
        let idx = self.board_list.get_selected_index().unwrap_or(0);
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
        self.reload_model();
        self.prepare_frame();
        // Clamp the highlight to the shrunken archived list, preserving
        // position (not identity — the removed board no longer exists there).
        let remaining = self.model.archived_boards_view().count();
        self.board_list
            .inner_mut()
            .set_selected_index((remaining > 0).then(|| idx.min(remaining - 1)));
        self.needs_redraw = true;
    }

    /// Permanently delete the highlighted archived board and its subtree (direct,
    /// called after the `DeletePermanentBoardConfirm` dialog confirms).
    pub fn handle_delete_archived_board(&mut self) {
        let Some(board_id) = self.selected_archived_board_id() else {
            return;
        };
        let idx = self.board_list.get_selected_index().unwrap_or(0);
        if let Err(e) = self.ctx.delete_board(board_id) {
            tracing::error!("Failed to delete archived board: {}", e);
            self.set_error(format!("Failed to delete archived board: {}", e));
            return;
        }
        tracing::info!("Permanently deleted archived board {}", board_id);
        self.reload_model();
        self.prepare_frame();
        let remaining = self.model.archived_boards_view().count();
        self.board_list
            .inner_mut()
            .set_selected_index((remaining > 0).then(|| idx.min(remaining - 1)));
        self.needs_redraw = true;
    }

    pub fn create_board(&mut self) {
        let board_name = self.input.as_str().to_string();

        let board_id = uuid::Uuid::new_v4();
        let position = self.model.live_boards().count() as i32;

        let mut commands: Vec<Command> = vec![Command::Board(BoardCommand::Create(CreateBoard {
            id: board_id,
            name: board_name.clone(),
            card_prefix: None,
            position,
        }))];

        for (position, (name, default_status)) in kanban_domain::DEFAULT_TEMPLATE_COLUMNS
            .into_iter()
            .enumerate()
        {
            let column_id = uuid::Uuid::new_v4();
            commands.push(Command::Column(ColumnCommand::Create(CreateColumn {
                id: column_id,
                board_id,
                name: name.to_string(),
                position: position as i32,
                default_status,
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

        // Resync `board_list` from the store so the new board is present, then
        // select it by id (known up front — it was generated above).
        self.reload_model();
        self.prepare_frame();
        self.board_list.select_board(board_id);
        self.switch_view_strategy(TaskListView::default());
    }

    pub fn rename_board(&mut self) {
        if let Some(board_id) = self.board_list.get_selected_board_id() {
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

            self.reload_model();
            tracing::info!("Renamed board to: {}", new_name);
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
    use kanban_domain::{
        BoardUpdate, CreateCardOptions, KanbanOperations, SortOrder, TaskListView,
    };

    /// Pull the store snapshot into `app.model` and resync `app.board_list` so
    /// handlers that read either observe prior writes (the event loop does
    /// this per frame via `prepare_frame`).
    fn refresh(app: &mut App) {
        app.reload_model();
        app.prepare_frame();
    }

    fn create_named_board(app: &mut App, name: &str) {
        app.input.set(name.to_string());
        app.create_board();
        app.input.clear();
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
    fn test_create_board_seeds_default_statuses() {
        // The default template seeds TODO/Doing/Complete, each carrying its own
        // `default_status`; the lifecycle sync is driven by `default_status`
        // alone.
        use kanban_domain::CardStatus;

        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");

        let board = app.ctx.data_store().list_boards().unwrap().remove(0);
        let cols = app.ctx.data_store().list_all_columns().unwrap();
        let by_name = |name: &str| {
            cols.iter()
                .find(|c| c.board_id == board.id && c.name == name)
                .unwrap_or_else(|| panic!("template column {name}"))
        };
        assert_eq!(by_name("TODO").default_status, Some(CardStatus::Todo));
        assert_eq!(
            by_name("Doing").default_status,
            Some(CardStatus::InProgress)
        );
        assert_eq!(by_name("Complete").default_status, Some(CardStatus::Done));
    }

    #[test]
    fn test_fresh_template_board_syncs_done_with_no_setup_step() {
        // The journey itself, with NOTHING between create and use: no board
        // update, no configuration command. Marking a card done on a board
        // fresh out of the create dialog must land it in Complete, and moving
        // it into Complete must mark it done.
        use kanban_domain::{CardStatus, CardUpdate, CreateCardOptions, KanbanOperations};

        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");

        let board = app.ctx.data_store().list_boards().unwrap().remove(0);
        let cols = app.ctx.data_store().list_all_columns().unwrap();
        let todo = cols
            .iter()
            .find(|c| c.board_id == board.id && c.name == "TODO")
            .unwrap()
            .id;
        let complete = cols
            .iter()
            .find(|c| c.board_id == board.id && c.name == "Complete")
            .unwrap()
            .id;

        let card = app
            .ctx
            .create_card(board.id, todo, "Task".into(), CreateCardOptions::default())
            .unwrap();
        let updated = app
            .ctx
            .update_card(
                card.id,
                CardUpdate {
                    status: Some(CardStatus::Done),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(updated.status, CardStatus::Done);
        assert_eq!(
            updated.column_id, complete,
            "status=done must land in Complete on a fresh board, with no setup step"
        );

        let moved = app.ctx.move_card(card.id, complete, None).unwrap();
        assert_eq!(
            moved.status,
            CardStatus::Done,
            "moving into Complete must not reset the status"
        );
    }

    #[test]
    fn test_undo_board_creation_still_reverses_the_full_seed() {
        // Board creation remains one undoable batch: the board and every
        // template column (each carrying its own seeded `default_status`)
        // must vanish together on undo, even though no separate
        // `completion_column_ids` update is issued anymore.
        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");

        assert!(app.ctx.undo().unwrap(), "undo applies");
        assert!(
            app.ctx.data_store().list_boards().unwrap().is_empty(),
            "the whole creation batch reverses in one step"
        );
        assert!(
            app.ctx.data_store().list_all_columns().unwrap().is_empty(),
            "the seeded default-status columns reverse with the board"
        );
    }

    #[test]
    fn test_undo_board_creation_reverses_seeded_default_statuses() {
        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");

        assert!(app.ctx.undo().unwrap(), "undo applies");
        assert!(
            app.ctx.data_store().list_all_columns().unwrap().is_empty(),
            "the whole creation batch, including the seeded columns, reverses in one step"
        );
    }

    #[test]
    fn test_delete_board_key_on_boards_opens_delete_board_confirm() {
        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");
        app.focus.active = Focus::Boards;
        app.board_list.inner_mut().set_selected_index(Some(0));

        app.handle_delete_board_key();

        assert_eq!(app.mode, AppMode::Dialog(DialogMode::DeleteBoardConfirm));
    }

    #[test]
    fn test_delete_empty_board_removes_board() {
        let mut app = App::test_default();
        create_named_board(&mut app, "Roadmap");
        let board_id = app.ctx.data_store().list_boards().unwrap()[0].id;
        app.focus.active = Focus::Boards;
        app.board_list.inner_mut().set_selected_index(Some(0));
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
        app.board_list.inner_mut().set_selected_index(Some(0));

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
            app.board_list.inner_mut().set_selected_index(Some(0));
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
        app.board_list.inner_mut().set_selected_index(Some(0));
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
        app.board_list.inner_mut().set_selected_index(Some(2));

        app.delete_board();
        assert_eq!(
            app.board_list.get_selected_index(),
            Some(1),
            "clamped to last survivor"
        );

        // Delete the remaining two; selection must clear at zero.
        app.board_list.inner_mut().set_selected_index(Some(1));
        app.delete_board();
        app.board_list.inner_mut().set_selected_index(Some(0));
        app.delete_board();
        assert_eq!(
            app.board_list.get_selected_index(),
            None,
            "selection cleared at zero"
        );
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
        use crate::view_strategy::UnifiedViewStrategy;
        use kanban_view::layout_strategy::ColumnListsLayout;
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
        app.board_list.inner_mut().set_selected_index(Some(3));

        app.delete_board();

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
        app.board_list.inner_mut().set_selected_index(Some(0)); // highlight A

        app.delete_board(); // archive A (before the active one)

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
        app.board_list.inner_mut().set_selected_index(Some(1)); // highlight B (the active board)

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
        app.board_list.inner_mut().set_selected_index(Some(1)); // highlight B

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
        app.board_list.inner_mut().set_selected_index(Some(0)); // highlight A (before C)

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
        app.board_list.inner_mut().set_selected_index(Some(0));
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
        app.board_list.inner_mut().set_selected_index(Some(0));
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
        app.board_list.inner_mut().set_selected_index(Some(0));

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
        app.reload_model();
        app.prepare_frame();
        app.focus.active = Focus::Boards;
        app.board_list.inner_mut().set_selected_index(Some(0));
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
        app.reload_model();
        app.prepare_frame();
        app.focus.active = Focus::Boards;
        app.board_list.inner_mut().set_selected_index(Some(0));

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
        app.reload_model();
        app.prepare_frame();
        app.focus.active = Focus::Boards;
        app.board_list.inner_mut().set_selected_index(Some(0));

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
        app.reload_model();
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
        app.reload_model();
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

    /// The shared SortOrder toggle (`s`) reverses the archived projects order
    /// while leaving the live view's persisted sort preference untouched: the
    /// archived and live sort pairs are independent, so a live Name preference
    /// does not leak into the archived default, and toggling the archived
    /// order does not overwrite the live preference in AppConfig.
    #[test]
    fn test_archived_boards_view_s_toggles_its_own_order_independent_of_live() {
        use std::str::FromStr;
        let mut app = App::test_default();
        let cfg_dir = tempfile::tempdir().unwrap();
        let cfg_path = cfg_dir.path().join("config.toml");
        app.app_config.configuration_location = Some(cfg_path.display().to_string());
        let (arch1, _) = seed_archived_board_with_cards(&mut app, "Arch1");
        let (arch2, _) = seed_archived_board_with_cards(&mut app, "Arch2");

        // Persist a LIVE sort preference (Name/Ascending) via the real handler
        // path, exactly like a user setting it from the live projects panel.
        app.mode = AppMode::Normal;
        app.focus.active = Focus::Boards;
        app.selection.active_board_id = None;
        app.apply_board_sort(kanban_domain::BoardSortField::Name, SortOrder::Ascending);
        assert!(
            cfg_path.exists(),
            "live sort persisted to config (precondition)"
        );

        // Switch to the archived view: it must stay on its own recency
        // default (Arch2 archived more recently), unaffected by the live
        // Name preference.
        app.mode = AppMode::ArchivedBoardsView;
        app.reload_model();
        app.prepare_frame();
        app.board_list.inner_mut().set_selected_index(Some(0));
        let rendered: Vec<uuid::Uuid> = app.displayed_boards().iter().map(|b| b.id).collect();
        assert_eq!(
            rendered,
            vec![arch2, arch1],
            "archived view keeps its own recency default, unaffected by the live Name sort"
        );

        // Toggle order via the shared 's' while viewing the archived list.
        app.board_list.inner_mut().set_selected_index(Some(0));
        app.handle_archived_boards_view_mode(KeyCode::Char('s'));

        // Recency ASC → oldest (Arch1) first: only the archived partition moved.
        let rendered: Vec<uuid::Uuid> = app.displayed_boards().iter().map(|b| b.id).collect();
        assert_eq!(
            rendered,
            vec![arch1, arch2],
            "'s' reverses only the archived partition's own order"
        );

        // The live preference is completely untouched by the archived-view
        // toggle: still Name/Ascending, not overwritten.
        assert_eq!(
            app.app_config.board_sort_field.as_deref(),
            Some("name"),
            "live sort field in AppConfig is unaffected by the archived-view toggle"
        );
        assert_eq!(
            app.app_config
                .board_sort_order
                .as_deref()
                .and_then(|s| SortOrder::from_str(s).ok()),
            Some(SortOrder::Ascending),
            "live sort order in AppConfig must still be Ascending, not flipped by the archived toggle"
        );
    }

    /// The converse of the test above: changing the archived view's sort must
    /// not affect the live view's order or its persisted AppConfig preference.
    #[test]
    fn test_live_board_order_unaffected_by_archived_sort_change() {
        use kanban_view::selection_dialog::popup_index_of_board_sort_field;
        let mut app = App::test_default();
        let cfg_dir = tempfile::tempdir().unwrap();
        app.app_config.configuration_location =
            Some(cfg_dir.path().join("config.toml").display().to_string());
        create_named_board(&mut app, "Zed");
        create_named_board(&mut app, "Alpha");
        seed_archived_board_with_cards(&mut app, "Arch1");
        seed_archived_board_with_cards(&mut app, "Arch2");

        // Live default is Position ASC: Zed (created first) then Alpha.
        app.mode = AppMode::Normal;
        app.focus.active = Focus::Boards;
        app.selection.active_board_id = None;
        app.reload_model();
        app.prepare_frame();
        app.board_list.inner_mut().set_selected_index(Some(0));
        let live_before: Vec<String> = app
            .displayed_boards()
            .iter()
            .map(|b| b.name.clone())
            .collect();
        assert_eq!(
            live_before,
            vec!["Zed", "Alpha"],
            "live default is Position ASC (precondition)"
        );

        // Change the ARCHIVED sort to Name ascending via the picker, from
        // within the archived view.
        app.mode = AppMode::ArchivedBoardsView;
        app.reload_model();
        app.prepare_frame();
        app.board_list.inner_mut().set_selected_index(Some(0));
        app.handle_archived_boards_view_mode(KeyCode::Char('o'));
        let name_idx = popup_index_of_board_sort_field(kanban_domain::BoardSortField::Name);
        app.filter.board_sort_field_selection.set(Some(name_idx));
        app.handle_order_boards_popup(KeyCode::Char('a'));

        // Switch back to the live view: order and persisted config unaffected.
        app.mode = AppMode::Normal;
        app.reload_model();
        app.prepare_frame();
        let live_after: Vec<String> = app
            .displayed_boards()
            .iter()
            .map(|b| b.name.clone())
            .collect();
        assert_eq!(
            live_after,
            vec!["Zed", "Alpha"],
            "live order unaffected by the archived sort change"
        );
        assert_eq!(
            app.app_config.board_sort_field, None,
            "archived-only sort change must not write the live AppConfig field"
        );
    }

    /// The board-sort field picker (`o`) opens over the archived-boards view and
    /// picking Recency (ArchivedAt) re-sorts the archived partition to
    /// newest-archived first.
    #[test]
    fn test_order_boards_picker_recency_orders_newest_first() {
        use kanban_view::selection_dialog::popup_index_of_board_sort_field;
        let mut app = App::test_default();
        let cfg_dir = tempfile::tempdir().unwrap();
        app.app_config.configuration_location =
            Some(cfg_dir.path().join("config.toml").display().to_string());
        let (arch1, _) = seed_archived_board_with_cards(&mut app, "Arch1");
        let (arch2, _) = seed_archived_board_with_cards(&mut app, "Arch2");

        app.mode = AppMode::ArchivedBoardsView;
        app.reload_model();
        app.prepare_frame();
        app.focus.active = Focus::Boards;
        app.board_list.inner_mut().set_selected_index(Some(0));

        // Open the picker over the archived view.
        app.handle_archived_boards_view_mode(KeyCode::Char('o'));
        assert_eq!(app.mode, AppMode::Dialog(DialogMode::OrderBoards));

        // Select the Recency row and confirm with 'd' (descending → newest first).
        let recency_idx =
            popup_index_of_board_sort_field(kanban_domain::BoardSortField::ArchivedAt);
        app.filter.board_sort_field_selection.set(Some(recency_idx));
        app.handle_order_boards_popup(KeyCode::Char('d'));

        assert_eq!(app.mode, AppMode::ArchivedBoardsView, "picker closed");
        let rendered: Vec<uuid::Uuid> = app.displayed_boards().iter().map(|b| b.id).collect();
        assert_eq!(
            rendered,
            vec![arch2, arch1],
            "Recency DESC orders newest-archived (Arch2) first"
        );
    }

    /// The LIVE projects panel is now sortable (KAN-955): with Normal mode +
    /// Boards focus, the `o` field picker opens and applying Name re-sorts the
    /// live list alphabetically. Proves the handler path reachable from the live
    /// `NormalModeBoardsProvider` binding actually works end to end.
    #[test]
    fn test_board_sort_reachable_on_live_panel() {
        use kanban_view::selection_dialog::popup_index_of_board_sort_field;
        let mut app = App::test_default();
        let cfg_dir = tempfile::tempdir().unwrap();
        app.app_config.configuration_location =
            Some(cfg_dir.path().join("config.toml").display().to_string());
        create_named_board(&mut app, "Zed");
        create_named_board(&mut app, "Alpha");

        app.mode = AppMode::Normal;
        app.focus.active = Focus::Boards;
        app.selection.active_board_id = None;
        app.board_list.inner_mut().set_selected_index(Some(0));

        // Open the picker via the live-panel `o` handler.
        app.handle_order_boards_key();
        assert_eq!(
            app.mode,
            AppMode::Dialog(DialogMode::OrderBoards),
            "the live panel's 'o' opens the board-sort picker"
        );

        // Pick Name, confirm ascending ('a').
        let name_idx = popup_index_of_board_sort_field(kanban_domain::BoardSortField::Name);
        app.filter.board_sort_field_selection.set(Some(name_idx));
        app.handle_order_boards_popup(KeyCode::Char('a'));

        assert_eq!(app.mode, AppMode::Normal, "picker closed back to Normal");
        let names: Vec<String> = app
            .displayed_boards()
            .iter()
            .map(|b| b.name.clone())
            .collect();
        assert_eq!(
            names,
            vec!["Alpha".to_string(), "Zed".to_string()],
            "the live projects panel is sorted alphabetically by Name"
        );
    }

    /// Once an archived board is ACTIVATED (mode stays ArchivedBoardsView but a
    /// board is active and focus is on Cards), the board-sort keys must be inert:
    /// they operate on the board LIST, not while viewing a board's contents
    /// (KAN-955 focus guard).
    #[test]
    fn test_board_sort_keys_inert_when_archived_board_activated() {
        let mut app = App::test_default();
        let (arch1, _) = seed_archived_board_with_cards(&mut app, "Arch1");
        let (arch2, _) = seed_archived_board_with_cards(&mut app, "Arch2");

        // Sort by Name ASC so the two boards have a stable order to observe.
        app.model.set_board_sort(
            true,
            kanban_domain::BoardSortField::Name,
            SortOrder::Ascending,
        );
        app.mode = AppMode::ArchivedBoardsView;
        app.reload_model();
        app.prepare_frame();

        // Simulate a board being ACTIVATED (drilled into): active id set, focus
        // on the tasks panel — the mode is still ArchivedBoardsView.
        app.selection.active_board_id = Some(arch1);
        app.focus.active = Focus::Cards;

        let before = app.model.board_sort(true);
        app.handle_toggle_board_sort_order();
        assert_eq!(
            app.model.board_sort(true),
            before,
            "'s' is inert while an archived board is activated (focus on Cards)"
        );

        app.handle_order_boards_key();
        assert_ne!(
            app.mode,
            AppMode::Dialog(DialogMode::OrderBoards),
            "'o' must not open the picker while a board is activated"
        );

        // Sanity: sort still fires when browsing the list (no active board).
        app.selection.active_board_id = None;
        app.focus.active = Focus::Boards;
        app.board_list.inner_mut().set_selected_index(Some(0));
        app.handle_toggle_board_sort_order();
        assert_eq!(
            app.model.board_sort(true).1,
            SortOrder::Descending,
            "'s' fires again once browsing the archived board list"
        );
        let _ = (arch1, arch2);
    }

    /// Data-loss guard: once an archived board is ACTIVATED (drilled into —
    /// `active_board_id` set, focus on Cards, mode still ArchivedBoardsView),
    /// pressing `x` must NOT permanently delete the board highlighted in the
    /// underlying list. `x` operates on the board LIST, not while viewing a
    /// board's contents.
    #[test]
    fn test_archived_board_drilled_in_x_does_not_delete_list_board() {
        let mut app = App::test_default();
        let (arch1, _) = seed_archived_board_with_cards(&mut app, "Arch1");
        let (arch2, _) = seed_archived_board_with_cards(&mut app, "Arch2");
        app.mode = AppMode::ArchivedBoardsView;
        app.reload_model();
        app.prepare_frame();

        // Drill into an archived board: active id set, focus on the tasks panel.
        app.selection.active_board_id = Some(arch1);
        app.focus.active = Focus::Cards;
        app.board_list.inner_mut().set_selected_index(Some(0));

        let archived_before = app.ctx.list_archived_boards().unwrap().len();
        app.handle_archived_boards_view_mode(KeyCode::Char('x'));

        assert_ne!(
            app.mode,
            AppMode::Dialog(DialogMode::DeletePermanentBoardConfirm),
            "x must not open the permanent-delete dialog while drilled into a board"
        );
        assert_eq!(
            app.ctx.list_archived_boards().unwrap().len(),
            archived_before,
            "no archived board removed while drilled in"
        );
        assert!(
            app.ctx
                .list_archived_boards()
                .unwrap()
                .iter()
                .any(|ab| ab.entity_id == arch1),
            "the activated board is still archived (not deleted)"
        );
        let _ = arch2;
    }

    /// Companion guard: `r` (restore) must be inert while drilled into an
    /// archived board — it would otherwise restore the wrong (highlighted) board
    /// out from under the user.
    #[test]
    fn test_archived_board_drilled_in_r_does_not_restore_list_board() {
        let mut app = App::test_default();
        let (arch1, _) = seed_archived_board_with_cards(&mut app, "Arch1");
        let (arch2, _) = seed_archived_board_with_cards(&mut app, "Arch2");
        app.mode = AppMode::ArchivedBoardsView;
        app.reload_model();
        app.prepare_frame();

        app.selection.active_board_id = Some(arch1);
        app.focus.active = Focus::Cards;
        app.board_list.inner_mut().set_selected_index(Some(0));

        let archived_before = app.ctx.list_archived_boards().unwrap().len();
        app.handle_archived_boards_view_mode(KeyCode::Char('r'));

        assert_eq!(
            app.ctx.list_archived_boards().unwrap().len(),
            archived_before,
            "no archived board restored while drilled in"
        );
        assert!(
            app.ctx
                .list_archived_boards()
                .unwrap()
                .iter()
                .any(|ab| ab.entity_id == arch1)
                && app
                    .ctx
                    .list_archived_boards()
                    .unwrap()
                    .iter()
                    .any(|ab| ab.entity_id == arch2),
            "both archived boards remain archived (nothing restored)"
        );
    }
}
