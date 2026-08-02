//! I6 (KAN-888): the TUI board archived surface — the ArchivedBoardsView mode,
//! its toggle, and the restore / permanent-delete affordances. Mirrors the
//! archived-cards view tests (`archive_delete_tests.rs`) but for boards, which
//! use direct restore/delete (no animation / multi-select).

use kanban_domain::KanbanOperations;
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::App;

/// Create a board via the ctx, archive it, refresh the model. Returns its id.
fn seed_archived_board(app: &mut App, name: &str) -> uuid::Uuid {
    let board = app.ctx.create_board(name.to_string(), None).unwrap();
    app.ctx.archive_board(board.id).unwrap();
    app.prepare_frame();
    board.id
}

#[test]
fn test_toggle_into_archived_boards_view_and_back() {
    let mut app = App::test_default();
    let _live = app.ctx.create_board("Live".to_string(), None).unwrap();
    let archived_id = seed_archived_board(&mut app, "Archived");

    // From the live boards panel, `D` (toggle) enters the archived view.
    app.focus.active = Focus::Boards;
    app.mode = AppMode::Normal;
    app.prepare_frame();
    // The live boards view (unified collection filtered by the archived-id set)
    // excludes the archived head, even though `boards()` now carries it.
    assert!(app.model.boards().iter().any(|b| b.id == archived_id));
    assert!(app.displayed_boards().iter().all(|b| b.id != archived_id));

    app.handle_toggle_archived_boards_view();
    assert_eq!(app.mode, AppMode::ArchivedBoardsView);
    // The archived view shows the archived board head.
    assert_eq!(app.displayed_boards().len(), 1);
    assert_eq!(app.displayed_boards()[0].id, archived_id);

    // Toggling again returns to the live boards view.
    app.handle_toggle_archived_boards_view();
    assert_eq!(app.mode, AppMode::Normal);
    assert!(app.displayed_boards().iter().any(|b| b.name == "Live"));
}

#[test]
fn test_q_in_archived_boards_view_returns_to_normal() {
    let mut app = App::test_default();
    seed_archived_board(&mut app, "Archived");
    app.focus.active = Focus::Boards;
    app.mode = AppMode::ArchivedBoardsView;
    app.prepare_frame();

    app.handle_archived_boards_view_mode(crossterm::event::KeyCode::Char('q'));
    assert_eq!(
        app.mode,
        AppMode::Normal,
        "'q' in ArchivedBoardsView returns to the live boards view"
    );
    assert!(!app.should_quit, "'q' in ArchivedBoardsView must not quit");
}

#[test]
fn test_restore_from_archived_boards_view_returns_board_to_live() {
    let mut app = App::test_default();
    let archived_id = seed_archived_board(&mut app, "Restore Me");

    app.focus.active = Focus::Boards;
    app.mode = AppMode::ArchivedBoardsView;
    app.prepare_frame();
    app.selection.board.set(Some(0));

    // `r` restores the highlighted archived board.
    app.handle_archived_boards_view_mode(crossterm::event::KeyCode::Char('r'));

    // Back in the live set, gone from the archived collection.
    assert!(
        app.ctx
            .list_boards()
            .unwrap()
            .iter()
            .any(|b| b.id == archived_id),
        "restored board is live again"
    );
    assert!(
        app.ctx
            .list_archived_boards()
            .unwrap()
            .iter()
            .all(|ab| ab.entity_id != archived_id),
        "restored board is no longer archived"
    );
}

#[test]
fn test_permanent_delete_from_archived_boards_view_removes_board() {
    let mut app = App::test_default();
    let archived_id = seed_archived_board(&mut app, "Delete Me");

    app.focus.active = Focus::Boards;
    app.mode = AppMode::ArchivedBoardsView;
    app.prepare_frame();
    app.selection.board.set(Some(0));

    // `x` opens the confirm dialog; confirming with Enter permanently deletes.
    app.handle_archived_boards_view_mode(crossterm::event::KeyCode::Char('x'));
    assert_eq!(
        app.mode,
        AppMode::Dialog(DialogMode::DeletePermanentBoardConfirm)
    );
    app.handle_delete_permanent_board_confirm_popup(crossterm::event::KeyCode::Enter);

    // Absent from BOTH the live and the archived collections.
    assert!(
        app.ctx
            .list_boards()
            .unwrap()
            .iter()
            .all(|b| b.id != archived_id),
        "permanently deleted board is not live"
    );
    assert!(
        app.ctx
            .list_archived_boards()
            .unwrap()
            .iter()
            .all(|ab| ab.entity_id != archived_id),
        "permanently deleted board is not archived"
    );
}

// KAN-906: confirm dialog before permanent delete

#[test]
fn test_x_in_archived_view_opens_confirm_not_immediate_delete() {
    let mut app = App::test_default();
    let archived_id = seed_archived_board(&mut app, "Delete Me");

    app.focus.active = Focus::Boards;
    app.mode = AppMode::ArchivedBoardsView;
    app.prepare_frame();
    app.selection.board.set(Some(0));

    // `x` must open the confirm dialog, NOT delete immediately.
    app.handle_archived_boards_view_mode(crossterm::event::KeyCode::Char('x'));

    assert_eq!(
        app.mode,
        AppMode::Dialog(DialogMode::DeletePermanentBoardConfirm),
        "x must open DeletePermanentBoardConfirm dialog"
    );
    // Board must still be in the archived list — not yet deleted.
    app.prepare_frame();
    assert!(
        app.model
            .archived_boards_view()
            .any(|b| b.id == archived_id),
        "board must not be deleted until user confirms"
    );
}

#[test]
fn test_confirm_permanent_delete_removes_board() {
    let mut app = App::test_default();
    let archived_id = seed_archived_board(&mut app, "Delete Me");

    app.focus.active = Focus::Boards;
    app.mode = AppMode::ArchivedBoardsView;
    app.prepare_frame();
    app.selection.board.set(Some(0));

    app.handle_archived_boards_view_mode(crossterm::event::KeyCode::Char('x'));
    assert_eq!(
        app.mode,
        AppMode::Dialog(DialogMode::DeletePermanentBoardConfirm)
    );

    // Confirming with 'y' should delete the board.
    app.handle_delete_permanent_board_confirm_popup(crossterm::event::KeyCode::Char('y'));
    app.prepare_frame();

    assert!(
        app.model
            .archived_boards_view()
            .all(|b| b.id != archived_id),
        "confirmed delete should permanently remove the board"
    );
    assert!(
        !matches!(
            app.mode,
            AppMode::Dialog(DialogMode::DeletePermanentBoardConfirm)
        ),
        "dialog should be dismissed after confirm"
    );
}

#[test]
fn test_cancel_permanent_delete_keeps_board() {
    let mut app = App::test_default();
    let archived_id = seed_archived_board(&mut app, "Keep Me");

    app.focus.active = Focus::Boards;
    app.mode = AppMode::ArchivedBoardsView;
    app.prepare_frame();
    app.selection.board.set(Some(0));

    app.handle_archived_boards_view_mode(crossterm::event::KeyCode::Char('x'));
    assert_eq!(
        app.mode,
        AppMode::Dialog(DialogMode::DeletePermanentBoardConfirm)
    );

    // Cancelling with 'n' must keep the board and dismiss dialog.
    app.handle_delete_permanent_board_confirm_popup(crossterm::event::KeyCode::Char('n'));
    app.prepare_frame();

    assert!(
        app.mode == AppMode::ArchivedBoardsView,
        "cancelling confirm must return to ArchivedBoardsView"
    );
    assert!(
        app.model
            .archived_boards_view()
            .any(|b| b.id == archived_id),
        "cancelled delete must keep the board archived"
    );
}

// KAN-903: key wiring for gg / G / u / U in ArchivedBoardsView

#[test]
fn test_archived_view_g_then_g_jumps_to_first() {
    let mut app = App::test_default();
    seed_archived_board(&mut app, "A");
    seed_archived_board(&mut app, "B");
    seed_archived_board(&mut app, "C");

    app.focus.active = Focus::Boards;
    app.mode = AppMode::ArchivedBoardsView;
    app.prepare_frame();
    // Start at the bottom.
    app.selection.board.set(Some(2));

    // `g` → pending, second `g` → jump to first.
    app.handle_archived_boards_view_mode(crossterm::event::KeyCode::Char('g'));
    app.handle_archived_boards_view_mode(crossterm::event::KeyCode::Char('g'));

    assert_eq!(
        app.selection.board.get(),
        Some(0),
        "gg should jump to the first item in the archived list"
    );
}

#[test]
fn test_archived_view_shift_g_jumps_to_last() {
    let mut app = App::test_default();
    seed_archived_board(&mut app, "A");
    seed_archived_board(&mut app, "B");
    seed_archived_board(&mut app, "C");

    app.focus.active = Focus::Boards;
    app.mode = AppMode::ArchivedBoardsView;
    app.prepare_frame();
    app.selection.board.set(Some(0));

    app.handle_archived_boards_view_mode(crossterm::event::KeyCode::Char('G'));

    assert_eq!(
        app.selection.board.get(),
        Some(2),
        "G should jump to the last item in the archived list"
    );
}

#[test]
fn test_archived_view_u_undoes_permanent_delete() {
    let mut app = App::test_default();
    let archived_id = seed_archived_board(&mut app, "Undo Me");

    app.focus.active = Focus::Boards;
    app.mode = AppMode::ArchivedBoardsView;
    app.prepare_frame();
    app.selection.board.set(Some(0));

    // Delete the archived board permanently via the confirm dialog.
    app.handle_archived_boards_view_mode(crossterm::event::KeyCode::Char('x'));
    assert_eq!(
        app.mode,
        AppMode::Dialog(DialogMode::DeletePermanentBoardConfirm)
    );
    app.handle_delete_permanent_board_confirm_popup(crossterm::event::KeyCode::Enter);
    app.prepare_frame();
    assert!(
        app.model.archived_boards_view().next().is_none(),
        "board must be gone after confirming permanent delete"
    );

    // `u` should undo the delete, bringing the board back.
    app.handle_archived_boards_view_mode(crossterm::event::KeyCode::Char('u'));
    app.prepare_frame();
    assert!(
        app.model
            .archived_boards_view()
            .any(|b| b.id == archived_id),
        "undo should restore the permanently deleted archived board"
    );
}
