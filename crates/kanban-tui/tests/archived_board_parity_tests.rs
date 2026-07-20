//! KAN-911: an archived board is substitutable for a live one everywhere.
//!
//! These tests drive an ARCHIVED board through the exact same handlers a LIVE
//! board uses — activation, card detail, board detail (settings/sprints/columns),
//! and a card action — and assert the behaviour is identical. There is no
//! archival-specific entry point: the only place the live/archived distinction
//! appears is the projects panel choosing which board SET to display.

use kanban_domain::{BoardUpdate, CreateCardOptions, KanbanOperations, TaskListView};
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::AppMode;
use kanban_tui::App;

/// Seed a board with two columns, two cards and a sprint, then archive the head.
/// Returns (board_id, first_column_id, first_card_id, sprint_id).
fn seed_and_archive_board(
    app: &mut App,
    name: &str,
) -> (uuid::Uuid, uuid::Uuid, uuid::Uuid, uuid::Uuid) {
    let board = app.ctx.create_board(name.to_string(), None).unwrap();
    let col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    app.ctx
        .create_column(board.id, "Doing".to_string(), None)
        .unwrap();
    let card1 = app
        .ctx
        .create_card(
            board.id,
            col.id,
            "Card1".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx
        .create_card(
            board.id,
            col.id,
            "Card2".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let sprint = app.ctx.create_sprint(board.id, None, None).unwrap();
    app.ctx.archive_board(board.id).unwrap();
    let snap = app.ctx.snapshot().unwrap();
    app.model.load_from_snapshot(snap);
    (board.id, col.id, card1.id, sprint.id)
}

/// Open the archived board through the SAME activation handler a live board
/// uses. Proof of reuse: `handle_selection_activate`, not a bespoke drilldown.
fn open_archived_board(app: &mut App) {
    app.mode = AppMode::ArchivedBoardsView;
    app.prepare_frame();
    app.focus.active = Focus::Boards;
    app.selection.board.set(Some(0));
    app.handle_selection_activate();
}

#[test]
fn test_archived_board_activates_by_id_like_live() {
    let mut app = App::test_default();
    let (board_id, _, _, _) = seed_and_archive_board(&mut app, "Arch");

    open_archived_board(&mut app);

    assert_eq!(
        app.selection.active_board_id,
        Some(board_id),
        "archived board becomes the active board, tracked by id"
    );
    assert_eq!(app.focus.active, Focus::Cards);
    // Its two live cards populate the tasks panel, exactly like a live board.
    let count = app
        .view
        .strategy
        .get_active_task_list()
        .map(|l| l.len())
        .unwrap_or(0);
    assert_eq!(count, 2, "archived board's tasks are shown");
    // The head stays archived — opening it does not restore it.
    assert!(app
        .ctx
        .list_archived_boards()
        .unwrap()
        .iter()
        .any(|ab| ab.entity_id == board_id));
}

#[test]
fn test_archived_board_card_detail_matches_live() {
    let mut app = App::test_default();
    let (_, _, _, _) = seed_and_archive_board(&mut app, "Arch");

    open_archived_board(&mut app);
    if let Some(list) = app.view.strategy.get_active_task_list_mut() {
        list.set_selected_index(Some(0));
    }

    // Same activation handler a live card uses — no archival branch.
    app.handle_selection_activate();

    assert_eq!(app.mode, AppMode::CardDetail);
    assert!(app.selection.active_card_id.is_some());
}

#[test]
fn test_archived_board_settings_view_reachable() {
    let mut app = App::test_default();
    let (board_id, _, _, _) = seed_and_archive_board(&mut app, "Arch");

    app.mode = AppMode::ArchivedBoardsView;
    app.prepare_frame();
    app.focus.active = Focus::Boards;
    app.selection.board.set(Some(0));

    // `e` opens board detail through the same handler as for a live board.
    app.handle_edit_board_key();

    assert_eq!(app.mode, AppMode::BoardDetail);
    assert_eq!(
        app.active_board().map(|b| b.id),
        Some(board_id),
        "board detail resolves the archived board by id"
    );
}

#[test]
fn test_archived_board_sprints_view_reachable() {
    let mut app = App::test_default();
    let (board_id, _, _, sprint_id) = seed_and_archive_board(&mut app, "Arch");

    app.mode = AppMode::ArchivedBoardsView;
    app.prepare_frame();
    app.focus.active = Focus::Boards;
    app.selection.board.set(Some(0));
    app.handle_edit_board_key();

    // The active board's sprints resolve archival-agnostically.
    let board = app.active_board().expect("archived board resolves");
    assert_eq!(board.id, board_id);
    let sprint_count = app
        .model
        .sprints()
        .iter()
        .filter(|s| s.board_id == board_id)
        .count();
    assert_eq!(sprint_count, 1, "archived board's sprint is visible");
    assert!(app.model.sprints().iter().any(|s| s.id == sprint_id));
}

#[test]
fn test_archived_board_columns_resolve() {
    let mut app = App::test_default();
    let (board_id, _, _, _) = seed_and_archive_board(&mut app, "Arch");

    app.mode = AppMode::ArchivedBoardsView;
    app.prepare_frame();
    app.focus.active = Focus::Boards;
    app.selection.board.set(Some(0));
    app.handle_edit_board_key();

    let board = app.active_board().expect("archived board resolves");
    assert_eq!(board.id, board_id);
    let column_count = app
        .model
        .columns()
        .iter()
        .filter(|c| c.board_id == board_id)
        .count();
    assert_eq!(column_count, 2, "archived board's columns are visible");
}

#[test]
fn test_archived_board_card_action_works() {
    let mut app = App::test_default();
    let (_, _, card_id, _) = seed_and_archive_board(&mut app, "Arch");

    open_archived_board(&mut app);
    if let Some(list) = app.view.strategy.get_active_task_list_mut() {
        list.set_selected_index(Some(0));
    }

    // Toggle completion via the SAME handler a live board's card uses.
    app.handle_toggle_card_completion();
    let snap = app.ctx.snapshot().unwrap();
    app.model.load_from_snapshot(snap);

    let card = app.model.card(card_id).expect("card still live");
    assert!(
        card.is_completed(),
        "card action on an archived board's card takes effect"
    );
}

#[test]
fn test_archived_board_kanban_view_honours_board_setting() {
    // A ColumnView archived board renders in kanban layout once active, exactly
    // like a live one — the layout follows the board, not its archival state.
    let mut app = App::test_default();
    let (board_id, _, _, _) = seed_and_archive_board(&mut app, "Arch");
    app.ctx
        .update_board(
            board_id,
            BoardUpdate {
                task_list_view: Some(TaskListView::ColumnView),
                ..Default::default()
            },
        )
        .unwrap();
    let snap = app.ctx.snapshot().unwrap();
    app.model.load_from_snapshot(snap);

    // Browsing the archived LIST is never kanban (the list must stay visible)...
    app.mode = AppMode::ArchivedBoardsView;
    app.focus.active = Focus::Boards;
    app.selection.board.set(Some(0));
    assert!(
        !app.is_kanban_view(),
        "the boards list itself is not kanban"
    );

    // ...but once the ColumnView board is active it IS kanban.
    open_archived_board(&mut app);
    assert!(
        app.is_kanban_view(),
        "an active ColumnView board is kanban, archived or not"
    );
}

#[test]
fn test_live_projects_panel_lists_live_boards_only() {
    // Guard: the live projects view excludes archived heads; they appear only
    // when the panel is toggled to the archived set.
    let mut app = App::test_default();
    app.ctx.create_board("Live".to_string(), None).unwrap();
    let (arch_id, _, _, _) = seed_and_archive_board(&mut app, "Arch");
    app.prepare_frame();

    // Normal mode: only the live board.
    app.mode = AppMode::Normal;
    let live: Vec<_> = app.displayed_boards().iter().map(|b| b.id).collect();
    assert!(
        !live.contains(&arch_id),
        "archived head hidden from live set"
    );
    assert_eq!(live.len(), 1, "only the live board is listed");

    // Toggled to archived: only the archived head.
    app.mode = AppMode::ArchivedBoardsView;
    let archived: Vec<_> = app.displayed_boards().iter().map(|b| b.id).collect();
    assert_eq!(
        archived,
        vec![arch_id],
        "archived set shows the archived head"
    );
}
