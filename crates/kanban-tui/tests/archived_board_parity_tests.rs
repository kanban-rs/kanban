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

    let card = app.model.card_by_id(card_id).expect("card still live");
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

/// When drilled into an archived board (mode still ArchivedBoardsView but a
/// board is activated and focus is on Cards), the keybinding provider must
/// advertise the CARD-list keys that actually work there (Enter detail, `e`
/// edit, `p` priority), not the board-list keys (restore/delete/nav). The
/// provider selection keys off `active_board_id`, mirroring the input router's
/// drill-in guard.
#[test]
fn test_archived_board_drillin_advertises_card_keys() {
    use kanban_tui::keybindings::{KeybindingAction, KeybindingRegistry};

    let mut app = App::test_default();
    let (_, _, _, _) = seed_and_archive_board(&mut app, "Arch");
    open_archived_board(&mut app);

    // Sanity: we are drilled in (active board set, focus on Cards, mode still
    // ArchivedBoardsView).
    assert!(app.selection.active_board_id.is_some());
    assert_eq!(app.focus.active, Focus::Cards);
    assert_eq!(app.mode, AppMode::ArchivedBoardsView);

    let ctx = KeybindingRegistry::get_provider(&app).get_context();
    let has = |action: KeybindingAction| ctx.bindings.iter().any(|b| b.action == action);

    assert!(
        has(KeybindingAction::SelectItem),
        "drill-in advertises Enter/Space detail (card key)"
    );
    assert!(
        has(KeybindingAction::EditCard),
        "drill-in advertises `e` edit (card key)"
    );
    assert!(
        has(KeybindingAction::SetCardPriority),
        "drill-in advertises `p` priority (card key)"
    );
    // Board-list ops must NOT be advertised while viewing a board's contents.
    assert!(
        !has(KeybindingAction::RestoreBoard) && !has(KeybindingAction::DeleteArchivedBoard),
        "board-list restore/delete keys are not advertised while drilled in"
    );
}

// --- KAN-958: the DRILL-IN key DISPATCH must match a live board, not just the
// advertised keybindings. The provider already lists card keys (test above);
// these drive the real dispatch (`handle_archived_boards_view_mode`) to prove
// the keys are actually HANDLED, closing the advertise/dispatch gap.

#[test]
fn test_archived_board_drill_in_esc_returns_to_list() {
    let mut app = App::test_default();
    seed_and_archive_board(&mut app, "Arch");
    open_archived_board(&mut app);

    assert!(app.selection.active_board_id.is_some());
    assert_eq!(app.focus.active, Focus::Cards);

    // Esc must back out to the archived boards LIST, exactly as it does for a
    // live board (handle_escape_key: clear active board, return focus to the
    // projects panel, which is still showing the archived set).
    app.handle_archived_boards_view_mode(crossterm::event::KeyCode::Esc);

    assert_eq!(
        app.selection.active_board_id, None,
        "Esc leaves the archived board, returning to the list"
    );
    assert_eq!(app.focus.active, Focus::Boards);
    assert_eq!(
        app.mode,
        AppMode::ArchivedBoardsView,
        "still browsing the archived set, now back at the list level"
    );
}

#[test]
fn test_archived_board_drill_in_archives_card_via_key() {
    let mut app = App::test_default();
    seed_and_archive_board(&mut app, "Arch");
    open_archived_board(&mut app);
    if let Some(list) = app.view.strategy.get_active_task_list_mut() {
        list.set_selected_index(Some(0));
    }
    let card_id = app.get_selected_card_id().expect("a card is selected");

    // `d` on the cards panel archives the focused card — identical to a live
    // board. Currently the drill-in routes to the boards-only handler, so `d`
    // is silently dropped.
    app.handle_archived_boards_view_mode(crossterm::event::KeyCode::Char('d'));

    assert!(
        app.animation.animating.contains_key(&card_id),
        "archiving a card on an archived board behaves like a live board"
    );
}

// --- KAN-970: `D`/`e`/`q` follow-on. KAN-958 fixed the DISPATCH delegation
// (drilled-in keys reach `handle_normal_key`), but `D`'s target handler and
// the `e`/`q` pre-intercepts still don't recognise `ArchivedBoardsView` as a
// valid origin, so they silently no-op even after delegation. This is why
// restoring an archived card on an archived board was unreachable: `D` never
// transitioned into the mode that exposes `r`/`x`.

#[test]
fn test_toggle_archived_cards_view_enters_from_drilled_in_archived_board() {
    let mut app = App::test_default();
    seed_and_archive_board(&mut app, "Arch");
    open_archived_board(&mut app);

    app.handle_toggle_archived_cards_view();

    assert_eq!(
        app.mode,
        AppMode::ArchivedCardsView,
        "`D` on a drilled-in archived board enters the archived-cards view, as it does for a live board"
    );
}

#[test]
fn test_toggle_archived_cards_view_returns_to_archived_board_not_normal() {
    let mut app = App::test_default();
    let (board_id, _, _, _) = seed_and_archive_board(&mut app, "Arch");
    open_archived_board(&mut app);
    app.handle_toggle_archived_cards_view();

    // Toggling back must return to the drilled-in ARCHIVED board, not fall
    // through to `Normal` — a naive fix that hardcodes `ArchivedCardsView =>
    // Normal` would strand the user in `Normal` with `active_board_id` still
    // set to an archived board.
    app.handle_toggle_archived_cards_view();

    assert_eq!(
        app.mode,
        AppMode::ArchivedBoardsView,
        "toggling back from the archived-cards view returns to the drilled-in archived board"
    );
    assert_eq!(
        app.selection.active_board_id,
        Some(board_id),
        "the archived board is still active after toggling back"
    );
}

#[test]
fn test_restore_card_reachable_from_drilled_in_archived_board() {
    use kanban_domain::KanbanOperations;

    let mut app = App::test_default();
    let board = app.ctx.create_board("Arch".to_string(), None).unwrap();
    let col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            col.id,
            "ArchiveMe".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.archive_card(card.id).unwrap();
    app.ctx.archive_board(board.id).unwrap();
    let snap = app.ctx.snapshot().unwrap();
    app.model.load_from_snapshot(snap);

    open_archived_board(&mut app);
    app.handle_archived_boards_view_mode(crossterm::event::KeyCode::Char('D'));
    assert_eq!(app.mode, AppMode::ArchivedCardsView);
    if let Some(list) = app.view.strategy.get_active_task_list_mut() {
        list.set_selected_index(Some(0));
    }

    app.handle_restore_card();

    // Restore is animation-driven (`start_restore_animation`), same as archive
    // in `test_archived_board_drill_in_archives_card_via_key` above — reaching
    // this state at all is the proof `r` is no longer dead here.
    assert!(
        app.animation.animating.contains_key(&card.id),
        "restoring an archived card is reachable from a drilled-in archived board"
    );
}

#[test]
fn test_q_backs_out_of_drilled_in_archived_board() {
    let mut app = App::test_default();
    seed_and_archive_board(&mut app, "Arch");
    open_archived_board(&mut app);

    app.handle_archived_boards_view_mode(crossterm::event::KeyCode::Char('q'));

    assert_eq!(
        app.selection.active_board_id, None,
        "`q` leaves the archived board, returning to the list, like Esc"
    );
    assert_eq!(app.focus.active, Focus::Boards);
    assert_eq!(app.mode, AppMode::ArchivedBoardsView);
}

#[test]
fn test_shift_q_backs_out_of_drilled_in_archived_board() {
    let mut app = App::test_default();
    seed_and_archive_board(&mut app, "Arch");
    open_archived_board(&mut app);

    app.handle_archived_boards_view_mode(crossterm::event::KeyCode::Char('Q'));

    assert_eq!(
        app.selection.active_board_id, None,
        "`Q` leaves the archived board, returning to the list, like Esc"
    );
    assert_eq!(app.focus.active, Focus::Boards);
    assert_eq!(app.mode, AppMode::ArchivedBoardsView);
}

#[test]
fn test_edit_key_active_true_when_drilled_into_archived_board() {
    let mut app = App::test_default();
    seed_and_archive_board(&mut app, "Arch");
    open_archived_board(&mut app);

    assert!(
        app.edit_key_active(),
        "`e` is eligible on a drilled-in archived board, as it is for a live board"
    );
}

#[test]
fn test_edit_key_active_false_when_browsing_archived_board_list() {
    let mut app = App::test_default();
    seed_and_archive_board(&mut app, "Arch");
    app.mode = AppMode::ArchivedBoardsView;
    app.focus.active = Focus::Boards;

    assert!(
        !app.edit_key_active(),
        "`e` is not eligible while merely browsing the archived-boards list (no active board)"
    );
}
