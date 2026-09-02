mod helpers;

use helpers::FailingSnapshotBackend;
use kanban_domain::{CreateCardOptions, KanbanOperations};
use kanban_tui::components::BannerVariant;
use kanban_tui::App;

#[test]
fn test_a_failed_reload_surfaces_an_error_to_the_user() {
    let mut app = App::test_default();
    app.ctx
        .create_board("Board".to_string(), None)
        .expect("create board");
    app.reload_model();
    assert!(app.ui_state.banner.is_none());

    let failing = FailingSnapshotBackend::wrap(app.ctx.backend());
    app.ctx.replace_backend(failing);
    app.reload_model();

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("failed reload must set an error banner");
    assert_eq!(banner.variant, BannerVariant::Error);
    assert!(banner.message.contains("Failed to load from store"));
}

#[test]
fn test_a_failed_reload_leaves_the_previous_model_contents() {
    let mut app = App::test_default();
    let board = app
        .ctx
        .create_board("Board".to_string(), None)
        .expect("create board");
    let column = app
        .ctx
        .create_column(board.id, "Column".to_string(), None)
        .expect("create column");
    app.ctx
        .create_card(
            board.id,
            column.id,
            "Card".to_string(),
            CreateCardOptions::default(),
        )
        .expect("create card");

    app.reload_model();
    let boards_before = app.model.boards_state().loaded_or_empty().len();
    let cards_before = app.model.cards_state().loaded_or_empty().len();
    assert_eq!(boards_before, 1);
    assert_eq!(cards_before, 1);

    let failing = FailingSnapshotBackend::wrap(app.ctx.backend());
    app.ctx.replace_backend(failing);
    app.reload_model();

    assert_eq!(app.model.boards_state().loaded_or_empty().len(), 1);
    assert_eq!(app.model.cards_state().loaded_or_empty().len(), 1);
    assert_eq!(app.model.boards_state().loaded_or_empty()[0].id, board.id);
}

#[test]
fn test_a_successful_reload_sets_no_banner() {
    let mut app = App::test_default();
    app.ctx
        .create_board("Board".to_string(), None)
        .expect("create board");
    app.reload_model();
    assert!(app.ui_state.banner.is_none());
}

#[test]
fn test_from_app_propagates_a_failed_snapshot_read() {
    let mut app = App::test_default();
    app.ctx
        .replace_backend(FailingSnapshotBackend::wrap(app.ctx.backend()));
    let result = app.ctx.snapshot();
    assert!(
        result.is_err(),
        "ctx.snapshot() must propagate a failed backend read, not fall back to Snapshot::default()"
    );
}
