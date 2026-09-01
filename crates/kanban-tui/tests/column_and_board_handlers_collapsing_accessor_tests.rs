//! `column_handlers.rs` and `board_handlers.rs` read several
//! `Model::columns()`/`Model::sprints()` call sites that collapse a
//! `NotLoaded` tier to an empty slice, so a stale read silently behaves as
//! "board has zero columns/sprints" instead of declining. These tests pin
//! the decline behaviour.

use crossterm::event::KeyCode;
use kanban_domain::{EntityIds, Invalidation, KanbanOperations};
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::app::{BoardFocus, Focus};
use kanban_tui::App;
use uuid::Uuid;

fn sync_model_from_store(app: &mut App) {
    let snapshot = app.ctx.snapshot().unwrap();
    app.load_snapshot(snapshot);
}

fn seed_board_with_two_columns(app: &mut App) -> (Uuid, Uuid, Uuid) {
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let first = app
        .ctx
        .create_column(board.id, "Todo".into(), Some(0))
        .unwrap();
    let second = app
        .ctx
        .create_column(board.id, "Doing".into(), Some(1))
        .unwrap();
    (board.id, first.id, second.id)
}

fn invalidate_columns_tier(app: &mut App) {
    let _ = app
        .model
        .invalidate(Invalidation::Entities(EntityIds::columns([Uuid::new_v4()])));
}

fn invalidate_sprints_tier(app: &mut App) {
    let _ = app
        .model
        .invalidate(Invalidation::Entities(EntityIds::sprints([Uuid::new_v4()])));
}

fn columns_for_board(app: &App, board_id: Uuid) -> Vec<uuid::Uuid> {
    app.ctx
        .data_store()
        .list_all_columns()
        .unwrap()
        .into_iter()
        .filter(|c| c.board_id == board_id)
        .map(|c| c.id)
        .collect()
}


#[test]
fn test_handle_delete_column_key_with_a_not_loaded_column_tier_declines() {
    let mut app = App::test_default();
    let (board_id, _first, _second) = seed_board_with_two_columns(&mut app);
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.focus.board_focus = BoardFocus::Columns;
    app.dialog_input.column_list.update_item_count(2);
    app.dialog_input.column_list.set_selected_index(Some(0));

    invalidate_columns_tier(&mut app);

    app.handle_delete_column_key();

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("declining a NotLoaded columns tier must set an error banner");
    assert!(banner.message.to_lowercase().contains("column"));
    assert!(
        !matches!(app.mode, AppMode::Dialog(DialogMode::DeleteColumnConfirm)),
        "the delete-column confirm dialog must not open on a declined columns tier"
    );
}

#[test]
fn test_handle_delete_column_key_still_opens_on_a_loaded_column_tier() {
    let mut app = App::test_default();
    let (board_id, _first, _second) = seed_board_with_two_columns(&mut app);
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.focus.board_focus = BoardFocus::Columns;
    app.dialog_input.column_list.update_item_count(2);
    app.dialog_input.column_list.set_selected_index(Some(0));

    app.handle_delete_column_key();

    assert!(matches!(
        app.mode,
        AppMode::Dialog(DialogMode::DeleteColumnConfirm)
    ));
    assert!(app.ui_state.banner.is_none());
}


#[test]
fn test_handle_move_column_up_with_a_not_loaded_column_tier_declines() {
    let mut app = App::test_default();
    let (board_id, first, second) = seed_board_with_two_columns(&mut app);
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.focus.board_focus = BoardFocus::Columns;
    app.dialog_input.column_list.update_item_count(2);
    app.dialog_input.column_list.set_selected_index(Some(1));

    invalidate_columns_tier(&mut app);

    app.handle_move_column_up();

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("declining a NotLoaded columns tier must set an error banner");
    assert!(banner.message.to_lowercase().contains("column"));

    let first_after = app.ctx.data_store().get_column(first).unwrap().unwrap();
    let second_after = app.ctx.data_store().get_column(second).unwrap().unwrap();
    assert_eq!(first_after.position, 0, "positions must not change");
    assert_eq!(second_after.position, 1, "positions must not change");
}

#[test]
fn test_handle_move_column_up_still_swaps_on_a_loaded_column_tier() {
    let mut app = App::test_default();
    let (board_id, first, second) = seed_board_with_two_columns(&mut app);
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.focus.board_focus = BoardFocus::Columns;
    app.dialog_input.column_list.update_item_count(2);
    app.dialog_input.column_list.set_selected_index(Some(1));

    app.handle_move_column_up();

    let first_after = app.ctx.data_store().get_column(first).unwrap().unwrap();
    let second_after = app.ctx.data_store().get_column(second).unwrap().unwrap();
    assert_eq!(first_after.position, 1);
    assert_eq!(second_after.position, 0);
    assert!(app.ui_state.banner.is_none());
}

#[test]
fn test_handle_move_column_down_with_a_not_loaded_column_tier_declines() {
    let mut app = App::test_default();
    let (board_id, first, second) = seed_board_with_two_columns(&mut app);
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.focus.board_focus = BoardFocus::Columns;
    app.dialog_input.column_list.update_item_count(2);
    app.dialog_input.column_list.set_selected_index(Some(0));

    invalidate_columns_tier(&mut app);

    app.handle_move_column_down();

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("declining a NotLoaded columns tier must set an error banner");
    assert!(banner.message.to_lowercase().contains("column"));

    let first_after = app.ctx.data_store().get_column(first).unwrap().unwrap();
    let second_after = app.ctx.data_store().get_column(second).unwrap().unwrap();
    assert_eq!(first_after.position, 0, "positions must not change");
    assert_eq!(second_after.position, 1, "positions must not change");
}

#[test]
fn test_handle_move_column_down_still_swaps_on_a_loaded_column_tier() {
    let mut app = App::test_default();
    let (board_id, first, second) = seed_board_with_two_columns(&mut app);
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.focus.board_focus = BoardFocus::Columns;
    app.dialog_input.column_list.update_item_count(2);
    app.dialog_input.column_list.set_selected_index(Some(0));

    app.handle_move_column_down();

    let first_after = app.ctx.data_store().get_column(first).unwrap().unwrap();
    let second_after = app.ctx.data_store().get_column(second).unwrap().unwrap();
    assert_eq!(first_after.position, 1);
    assert_eq!(second_after.position, 0);
    assert!(app.ui_state.banner.is_none());
}


#[test]
fn test_create_column_with_a_not_loaded_column_tier_declines() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    app.ctx
        .create_column(board.id, "Todo".into(), Some(0))
        .unwrap();
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board.id);

    invalidate_columns_tier(&mut app);

    let before = columns_for_board(&app, board.id).len();

    app.input.set("New Column".to_string());
    app.create_column();
    app.input.clear();

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("declining a NotLoaded columns tier must set an error banner");
    assert!(banner.message.to_lowercase().contains("column"));

    let after = columns_for_board(&app, board.id).len();
    assert_eq!(
        before, after,
        "no column must be created while the columns tier is declined"
    );
}

#[test]
fn test_create_column_still_creates_on_a_loaded_column_tier() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    app.ctx
        .create_column(board.id, "Todo".into(), Some(0))
        .unwrap();
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board.id);

    let before = columns_for_board(&app, board.id).len();

    app.input.set("New Column".to_string());
    app.create_column();
    app.input.clear();

    assert!(app.ui_state.banner.is_none());
    let after = columns_for_board(&app, board.id).len();
    assert_eq!(after, before + 1);
}


#[test]
fn test_delete_column_declines_when_the_column_tier_is_not_loaded() {
    let mut app = App::test_default();
    let (board_id, first, second) = seed_board_with_two_columns(&mut app);
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.focus.board_focus = BoardFocus::Columns;
    app.dialog_input.column_list.update_item_count(2);
    app.dialog_input.column_list.set_selected_index(Some(0));

    invalidate_columns_tier(&mut app);

    app.delete_column();

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("declining a NotLoaded columns tier must set an error banner");
    assert!(banner.message.to_lowercase().contains("column"));

    assert!(app.ctx.data_store().get_column(first).unwrap().is_some());
    assert!(app.ctx.data_store().get_column(second).unwrap().is_some());
}

#[test]
fn test_delete_column_still_deletes_on_a_loaded_column_tier() {
    let mut app = App::test_default();
    let (board_id, first, second) = seed_board_with_two_columns(&mut app);
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.focus.board_focus = BoardFocus::Columns;
    app.dialog_input.column_list.update_item_count(2);
    app.dialog_input.column_list.set_selected_index(Some(0));

    app.delete_column();

    assert!(app.ui_state.banner.is_none());
    assert!(app.ctx.data_store().get_column(first).unwrap().is_none());
    assert!(app.ctx.data_store().get_column(second).unwrap().is_some());
}


#[test]
fn test_handle_delete_board_key_declines_when_board_delete_counts_is_not_loaded() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    app.ctx
        .create_column(board.id, "Todo".into(), Some(0))
        .unwrap();
    sync_model_from_store(&mut app);
    app.prepare_frame();
    app.focus.active = Focus::Boards;
    app.board_list.inner_mut().set_selected_index(Some(0));

    invalidate_columns_tier(&mut app);

    app.handle_delete_board_key();

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("declining a NotLoaded columns tier must set an error banner");
    assert!(banner.message.to_lowercase().contains("column"));
    assert!(
        !matches!(app.mode, AppMode::Dialog(DialogMode::DeleteBoardConfirm)),
        "the delete-board confirm dialog must not open on a declined tier"
    );
}

#[test]
fn test_handle_delete_board_key_still_opens_on_a_loaded_model() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    app.ctx
        .create_column(board.id, "Todo".into(), Some(0))
        .unwrap();
    sync_model_from_store(&mut app);
    app.prepare_frame();
    app.focus.active = Focus::Boards;
    app.board_list.inner_mut().set_selected_index(Some(0));

    app.handle_delete_board_key();

    assert!(matches!(
        app.mode,
        AppMode::Dialog(DialogMode::DeleteBoardConfirm)
    ));
    assert!(app.ui_state.banner.is_none());
}

#[test]
fn test_handle_delete_archived_board_key_declines_when_board_delete_counts_is_not_loaded() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    app.ctx
        .create_column(board.id, "Todo".into(), Some(0))
        .unwrap();
    app.ctx.archive_board(board.id).unwrap();
    sync_model_from_store(&mut app);
    app.mode = AppMode::ArchivedBoardsView;
    app.prepare_frame();
    app.focus.active = Focus::Boards;
    app.board_list.inner_mut().set_selected_index(Some(0));

    invalidate_columns_tier(&mut app);

    app.handle_delete_archived_board_key();

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("declining a NotLoaded columns tier must set an error banner");
    assert!(banner.message.to_lowercase().contains("column"));
    assert!(
        !matches!(
            app.mode,
            AppMode::Dialog(DialogMode::DeletePermanentBoardConfirm)
        ),
        "the permanent-delete confirm dialog must not open on a declined tier"
    );
}

#[test]
fn test_handle_delete_archived_board_key_still_opens_on_a_loaded_model() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    app.ctx
        .create_column(board.id, "Todo".into(), Some(0))
        .unwrap();
    app.ctx.archive_board(board.id).unwrap();
    sync_model_from_store(&mut app);
    app.mode = AppMode::ArchivedBoardsView;
    app.prepare_frame();
    app.focus.active = Focus::Boards;
    app.board_list.inner_mut().set_selected_index(Some(0));

    app.handle_delete_archived_board_key();

    assert!(matches!(
        app.mode,
        AppMode::Dialog(DialogMode::DeletePermanentBoardConfirm)
    ));
    assert!(app.ui_state.banner.is_none());
}


#[test]
fn test_all_nine_migrated_sites_still_produce_identical_results_on_a_fully_loaded_model() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let col_a = app
        .ctx
        .create_column(board.id, "A".into(), Some(0))
        .unwrap()
        .id;
    let col_b = app
        .ctx
        .create_column(board.id, "B".into(), Some(1))
        .unwrap()
        .id;
    let col_c = app
        .ctx
        .create_column(board.id, "C".into(), Some(2))
        .unwrap()
        .id;
    app.ctx.create_sprint(board.id, None, None).unwrap();
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board.id);
    app.prepare_frame();
    app.focus.active = Focus::Boards;
    app.focus.board_focus = BoardFocus::Columns;
    app.board_list.inner_mut().set_selected_index(Some(0));

    app.handle_delete_board_key();
    assert!(matches!(
        app.mode,
        AppMode::Dialog(DialogMode::DeleteBoardConfirm)
    ));
    app.handle_delete_board_confirm_popup(KeyCode::Esc);

    app.input.set("D".to_string());
    app.create_column();
    app.input.clear();
    let cols_after_create = columns_for_board(&app, board.id);
    assert_eq!(cols_after_create.len(), 4);

    app.dialog_input.column_list.update_item_count(4);
    app.dialog_input.column_list.set_selected_index(Some(1));
    app.handle_move_column_up();
    let a_after = app.ctx.data_store().get_column(col_a).unwrap().unwrap();
    let b_after = app.ctx.data_store().get_column(col_b).unwrap().unwrap();
    assert_eq!(a_after.position, 1);
    assert_eq!(b_after.position, 0);

    app.dialog_input.column_list.set_selected_index(Some(2));
    app.delete_column();
    assert!(app.ctx.data_store().get_column(col_c).unwrap().is_none());

    assert!(app.ui_state.banner.is_none());
}
