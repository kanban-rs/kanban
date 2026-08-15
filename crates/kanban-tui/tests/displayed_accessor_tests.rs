//! KAN-933 (ARCH-DECOR T1c): the single consumption accessor pair
//! `App::displayed_cards()` / `App::displayed_boards()` that selects the
//! live-vs-archived subset of the unified collection, keyed on the STACK-AWARE
//! base mode (so a confirm dialog opened over an archived view still resolves
//! the archived set — the underlay-bug regression guard).

use kanban_domain::{CreateCardOptions, KanbanOperations};
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::App;

/// Seed a board with a live card and an archived card. Returns
/// (live_card_id, archived_card_id).
fn seed_live_and_archived_card(app: &mut App) -> (uuid::Uuid, uuid::Uuid) {
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let live = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Live".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let archived = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Archived".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.archive_card(archived.id).unwrap();
    app.selection.active_board_id = Some(board.id);
    app.reload_model();
    app.prepare_frame();
    (live.id, archived.id)
}

/// Seed a live board and an archived board. Returns (live_id, archived_id).
fn seed_live_and_archived_board(app: &mut App) -> (uuid::Uuid, uuid::Uuid) {
    let live = app.ctx.create_board("Live".to_string(), None).unwrap();
    let archived = app.ctx.create_board("Archived".to_string(), None).unwrap();
    app.ctx.archive_board(archived.id).unwrap();
    app.reload_model();
    app.prepare_frame();
    (live.id, archived.id)
}

#[test]
fn test_displayed_cards_liveonly_excludes_archived() {
    let mut app = App::test_default();
    let (live_id, archived_id) = seed_live_and_archived_card(&mut app);

    app.mode = AppMode::Normal;
    app.reload_model();
    app.prepare_frame();

    let displayed: Vec<uuid::Uuid> = app.displayed_cards().iter().map(|c| c.id).collect();
    assert!(
        displayed.contains(&live_id),
        "live view shows the live card"
    );
    assert!(
        !displayed.contains(&archived_id),
        "live view excludes the archived card"
    );
}

#[test]
fn test_displayed_cards_archived_view_only_archived() {
    let mut app = App::test_default();
    let (live_id, archived_id) = seed_live_and_archived_card(&mut app);

    app.mode = AppMode::ArchivedCardsView;
    app.reload_model();
    app.prepare_frame();

    let displayed: Vec<uuid::Uuid> = app.displayed_cards().iter().map(|c| c.id).collect();
    assert_eq!(
        displayed,
        vec![archived_id],
        "archived view shows only the archived card"
    );
    assert!(
        !displayed.contains(&live_id),
        "archived view excludes the live card"
    );
}

#[test]
fn test_displayed_boards_liveonly_excludes_archived() {
    let mut app = App::test_default();
    let (live_id, archived_id) = seed_live_and_archived_board(&mut app);

    app.mode = AppMode::Normal;
    app.reload_model();
    app.prepare_frame();

    let displayed: Vec<uuid::Uuid> = app.displayed_boards().iter().map(|b| b.id).collect();
    assert!(
        displayed.contains(&live_id),
        "live view shows the live board"
    );
    assert!(
        !displayed.contains(&archived_id),
        "live view excludes the archived board"
    );
}

#[test]
fn test_displayed_boards_archived_view_only_archived() {
    let mut app = App::test_default();
    let (live_id, archived_id) = seed_live_and_archived_board(&mut app);

    app.mode = AppMode::ArchivedBoardsView;
    app.reload_model();
    app.prepare_frame();

    let displayed: Vec<uuid::Uuid> = app.displayed_boards().iter().map(|b| b.id).collect();
    assert_eq!(
        displayed,
        vec![archived_id],
        "archived view shows only the archived board"
    );
    assert!(
        !displayed.contains(&live_id),
        "archived view excludes the live board"
    );
}

#[test]
fn test_displayed_boards_set_uses_base_mode_under_dialog() {
    // Underlay-bug regression guard: with a confirm dialog open OVER the archived
    // boards view (mode == Dialog(..), base == ArchivedBoardsView), the accessor
    // must still resolve the ARCHIVED set — not the live set that raw `app.mode`
    // would (wrongly) select while the modal is up.
    let mut app = App::test_default();
    let (live_id, archived_id) = seed_live_and_archived_board(&mut app);

    app.mode = AppMode::ArchivedBoardsView;
    app.reload_model();
    app.prepare_frame();
    // Open a confirm dialog over the archived view.
    app.open_dialog(DialogMode::DeletePermanentBoardConfirm);
    assert!(
        matches!(app.mode, AppMode::Dialog(_)),
        "dialog is the raw mode"
    );
    assert_eq!(
        app.get_base_mode(),
        &AppMode::ArchivedBoardsView,
        "base mode is still the archived view under the dialog"
    );

    let displayed: Vec<uuid::Uuid> = app.displayed_boards().iter().map(|b| b.id).collect();
    assert_eq!(
        displayed,
        vec![archived_id],
        "under a dialog, displayed_boards still yields the archived set (base mode)"
    );
    assert!(
        !displayed.contains(&live_id),
        "under a dialog, the live board must not leak into the archived view"
    );
}

#[test]
fn test_displayed_cards_set_uses_base_mode_under_dialog() {
    // Same underlay-bug guard for the card side: a confirm dialog over the
    // archived CARDS view keeps `displayed_cards()` on the archived subset.
    let mut app = App::test_default();
    let (live_id, archived_id) = seed_live_and_archived_card(&mut app);

    app.mode = AppMode::ArchivedCardsView;
    app.reload_model();
    app.prepare_frame();
    app.open_dialog(DialogMode::DeletePermanentBoardConfirm);
    assert!(matches!(app.mode, AppMode::Dialog(_)));
    assert_eq!(app.get_base_mode(), &AppMode::ArchivedCardsView);

    let displayed: Vec<uuid::Uuid> = app.displayed_cards().iter().map(|c| c.id).collect();
    assert_eq!(
        displayed,
        vec![archived_id],
        "under a dialog, displayed_cards still yields the archived set (base mode)"
    );
    assert!(!displayed.contains(&live_id));
}
