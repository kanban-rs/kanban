use crossterm::event::KeyCode;
use kanban_domain::{CardStatus, ColumnUpdate, CreateCardOptions, KanbanOperations};
use kanban_tui::app::focus::Focus;
use kanban_tui::app::{AppMode, BoardFocus, DialogMode};
use kanban_tui::App;

fn refresh(app: &mut App) {
    let snap = app.ctx.snapshot().unwrap();
    app.model.load_from_snapshot(snap);
}

fn setup_board_with_columns(app: &mut App) -> (uuid::Uuid, uuid::Uuid, uuid::Uuid) {
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let todo = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    let doing = app
        .ctx
        .create_column(board.id, "Doing".to_string(), Some(1))
        .unwrap();
    let done = app
        .ctx
        .create_column(board.id, "Done".to_string(), Some(2))
        .unwrap();
    app.selection.active_board_id = Some(board.id);
    refresh(app);
    (todo.id, doing.id, done.id)
}

fn board_columns(app: &App, board_id: uuid::Uuid) -> Vec<kanban_domain::Column> {
    kanban_domain::card_lifecycle::sorted_board_columns(board_id, app.model.columns())
        .into_iter()
        .cloned()
        .collect()
}

fn select_column(app: &mut App, board_id: uuid::Uuid, column_id: uuid::Uuid) {
    app.focus.board_focus = BoardFocus::Columns;
    let columns = board_columns(app, board_id);
    let idx = columns
        .iter()
        .position(|c| c.id == column_id)
        .expect("column visible");
    app.dialog_input
        .column_list
        .update_item_count(columns.len());
    app.dialog_input.column_list.set_selected_index(Some(idx));
}

#[test]
fn test_column_edit_dialog_shows_current_default_status() {
    let mut app = App::test_default();
    let (_todo, doing, _done) = setup_board_with_columns(&mut app);
    let board_id = app.selection.active_board_id.unwrap();

    app.ctx
        .update_column(
            doing,
            ColumnUpdate {
                default_status: Some(Some(CardStatus::InProgress)),
                ..Default::default()
            },
        )
        .unwrap();
    refresh(&mut app);

    select_column(&mut app, board_id, doing);
    app.handle_set_column_default_status_key();

    assert_eq!(
        app.mode,
        AppMode::Dialog(DialogMode::SetColumnDefaultStatus),
        "dialog must open"
    );
    let selected_idx = app
        .dialog_input
        .default_status_selection
        .get()
        .expect("selection snapshotted on open");
    let selected_status =
        kanban_view::selection_dialog::default_status_at_popup_index(selected_idx)
            .expect("index in range");
    assert_eq!(
        selected_status,
        Some(CardStatus::InProgress),
        "dialog must show the column's current default_status"
    );
}

#[test]
fn test_column_edit_dialog_sets_default_status() {
    let mut app = App::test_default();
    let (_todo, doing, _done) = setup_board_with_columns(&mut app);
    let board_id = app.selection.active_board_id.unwrap();

    select_column(&mut app, board_id, doing);
    app.handle_set_column_default_status_key();

    let target_idx =
        kanban_view::selection_dialog::popup_index_of_default_status(Some(CardStatus::InProgress));
    app.dialog_input
        .default_status_selection
        .set(Some(target_idx));
    app.handle_set_column_default_status_popup(KeyCode::Enter);

    let column = app.ctx.get_column(doing).unwrap().unwrap();
    assert_eq!(
        column.default_status,
        Some(CardStatus::InProgress),
        "Enter on the dialog must persist the selected default_status"
    );
    assert_eq!(app.mode, AppMode::Normal, "dialog closes after Enter");
}

#[test]
fn test_column_edit_dialog_clears_default_status() {
    let mut app = App::test_default();
    let (_todo, doing, _done) = setup_board_with_columns(&mut app);
    let board_id = app.selection.active_board_id.unwrap();

    app.ctx
        .update_column(
            doing,
            ColumnUpdate {
                default_status: Some(Some(CardStatus::InProgress)),
                ..Default::default()
            },
        )
        .unwrap();
    refresh(&mut app);

    select_column(&mut app, board_id, doing);
    app.handle_set_column_default_status_key();

    let none_idx = kanban_view::selection_dialog::popup_index_of_default_status(None);
    app.dialog_input
        .default_status_selection
        .set(Some(none_idx));
    app.handle_set_column_default_status_popup(KeyCode::Enter);

    let column = app.ctx.get_column(doing).unwrap().unwrap();
    assert_eq!(
        column.default_status, None,
        "selecting the (none) entry must clear default_status"
    );
}

#[test]
fn test_column_default_status_change_is_undoable() {
    let mut app = App::test_default();
    let (_todo, doing, _done) = setup_board_with_columns(&mut app);
    let board_id = app.selection.active_board_id.unwrap();

    select_column(&mut app, board_id, doing);
    app.handle_set_column_default_status_key();

    let target_idx =
        kanban_view::selection_dialog::popup_index_of_default_status(Some(CardStatus::Blocked));
    app.dialog_input
        .default_status_selection
        .set(Some(target_idx));
    app.handle_set_column_default_status_popup(KeyCode::Enter);

    let column = app.ctx.get_column(doing).unwrap().unwrap();
    assert_eq!(column.default_status, Some(CardStatus::Blocked));

    assert!(app.ctx.can_undo(), "the change must ride a command");
    assert!(app.ctx.undo().unwrap(), "undo must succeed");

    let restored = app.ctx.get_column(doing).unwrap().unwrap();
    assert_eq!(
        restored.default_status, None,
        "undo must restore the prior default_status (None)"
    );
}

#[test]
fn test_moving_card_into_default_status_column_updates_status_in_tui() {
    let mut app = App::test_default();
    let (todo, doing, _done) = setup_board_with_columns(&mut app);
    let board_id = app.selection.active_board_id.unwrap();

    app.ctx
        .update_column(
            doing,
            ColumnUpdate {
                default_status: Some(Some(CardStatus::InProgress)),
                ..Default::default()
            },
        )
        .unwrap();

    let card = app
        .ctx
        .create_card(
            board_id,
            todo,
            "Mover".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    assert_eq!(card.status, CardStatus::Todo);

    app.focus.active = Focus::Cards;
    refresh(&mut app);
    app.prepare_frame();
    app.select_card_by_id(card.id);

    app.handle_move_card_right();

    let moved = app.ctx.get_card(card.id).unwrap().unwrap();
    assert_eq!(moved.column_id, doing, "card must land in Doing");
    assert_eq!(
        moved.status,
        CardStatus::InProgress,
        "moving a Todo card into a default_status column must promote it"
    );

    assert!(app.ctx.can_undo());
    assert!(app.ctx.undo().unwrap());
    let restored = app.ctx.get_card(card.id).unwrap().unwrap();
    assert_eq!(restored.column_id, todo);
    assert_eq!(restored.status, CardStatus::Todo);
}
