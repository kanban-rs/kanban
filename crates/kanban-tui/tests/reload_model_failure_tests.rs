mod helpers;

use helpers::CountingBackend;
use kanban_domain::{CreateCardOptions, KanbanOperations};
use kanban_tui::app::mode::AppMode;
use kanban_tui::components::BannerVariant;
use kanban_tui::App;

fn assert_error_banner(app: &App) {
    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("failed reload must set an error banner");
    assert_eq!(banner.variant, BannerVariant::Error);
    assert!(banner.message.contains("Failed to load from store"));
}

#[test]
fn test_a_failed_reload_surfaces_an_error_to_the_user() {
    let mut app = App::test_default();
    app.ctx
        .create_board("Board".to_string(), None)
        .expect("create board");
    app.reload_model();
    assert!(app.ui_state.banner.is_none());

    let failing = CountingBackend::wrap_failing(app.ctx.backend(), "list_boards");
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

    app.selection.active_board_id = Some(board.id);
    app.reload_model();
    let boards_before = app.model.boards_state().loaded_or_empty().len();
    let cards_before = app
        .model
        .board_cards_state(board.id)
        .loaded()
        .map(|v| v.len())
        .unwrap_or(0);
    assert_eq!(boards_before, 1);
    assert_eq!(cards_before, 1);

    let failing = CountingBackend::wrap_failing(app.ctx.backend(), "list_boards");
    app.ctx.replace_backend(failing);
    app.reload_model();

    assert_eq!(app.model.boards_state().loaded_or_empty().len(), 1);
    assert_eq!(
        app.model
            .board_cards_state(board.id)
            .loaded()
            .map(|v| v.len())
            .unwrap_or(0),
        1
    );
    assert_eq!(app.model.boards_state().loaded_or_empty()[0].id, board.id);
}

#[test]
fn test_a_failed_graph_read_rolls_back_to_the_previous_model() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Column".to_string(), None)
        .unwrap();
    let parent = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Parent".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let child = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Child".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let (parent_id, child_id) = (parent.id, child.id);
    app.ctx
        .data_store()
        .modify_graph(Box::new(move |g| g.set_parent(child_id, parent_id)))
        .unwrap();

    app.selection.active_board_id = Some(board.id);
    app.selection.active_card_id = Some(child.id);
    app.mode = AppMode::CardDetail;
    app.reload_model();
    assert_eq!(
        app.model
            .graph_state()
            .loaded()
            .map(|g| g.spawns_edges().len()),
        Some(1),
        "fixture sanity: one spawns edge seeded"
    );

    let failing = CountingBackend::wrap_failing(app.ctx.backend(), "get_graph");
    app.ctx.replace_backend(failing);
    app.reload_model();

    assert_error_banner(&app);
    assert_eq!(
        app.model
            .graph_state()
            .loaded()
            .map(|g| g.spawns_edges().len()),
        Some(1),
        "the previous graph must survive a failed reload"
    );
}

#[test]
fn test_a_failed_scoped_column_read_rolls_back_to_the_previous_model() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    app.ctx
        .create_column(board.id, "Column".to_string(), None)
        .unwrap();

    app.selection.active_board_id = Some(board.id);
    app.reload_model();
    assert_eq!(
        app.model
            .board_columns_state(board.id)
            .loaded()
            .map(|v| v.len()),
        Some(1),
        "fixture sanity: one column seeded"
    );

    let failing = CountingBackend::wrap_failing(app.ctx.backend(), "list_columns_by_board");
    app.ctx.replace_backend(failing);
    app.reload_model();

    assert_error_banner(&app);
    assert_eq!(
        app.model
            .board_columns_state(board.id)
            .loaded()
            .map(|v| v.len()),
        Some(1),
        "the previous columns must survive a failed reload"
    );
}

#[test]
fn test_a_failed_scoped_sprint_read_rolls_back_to_the_previous_model() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Column".to_string(), None)
        .unwrap();
    app.ctx
        .create_card(
            board.id,
            column.id,
            "Card".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.create_sprint(board.id, None, None).unwrap();

    app.selection.active_board_id = Some(board.id);
    app.reload_model();
    assert_eq!(
        app.model
            .board_sprints_state(board.id)
            .loaded()
            .map(|v| v.len()),
        Some(1),
        "fixture sanity: one sprint seeded"
    );

    let failing = CountingBackend::wrap_failing(app.ctx.backend(), "list_sprints_by_board");
    app.ctx.replace_backend(failing);
    app.reload_model();

    assert_error_banner(&app);
    assert_eq!(
        app.model
            .board_sprints_state(board.id)
            .loaded()
            .map(|v| v.len()),
        Some(1),
        "the previous sprints must survive a failed reload"
    );
}

#[test]
fn test_a_failed_archived_board_read_rolls_back_to_the_previous_model() {
    let mut app = App::test_default();
    let live_board = app.ctx.create_board("Live".to_string(), None).unwrap();
    let archived_board = app.ctx.create_board("Archived".to_string(), None).unwrap();
    app.ctx.archive_board(archived_board.id).unwrap();

    app.selection.active_board_id = Some(live_board.id);
    app.mode = AppMode::ArchivedBoardsView;
    app.reload_model();
    assert_eq!(
        app.model.archived_boards_state().loaded().map(|v| v.len()),
        Some(1),
        "fixture sanity: one archived board seeded"
    );

    let failing = CountingBackend::wrap_failing(app.ctx.backend(), "list_archived_boards");
    app.ctx.replace_backend(failing);
    app.reload_model();

    assert_error_banner(&app);
    assert_eq!(
        app.model.archived_boards_state().loaded().map(|v| v.len()),
        Some(1),
        "the previous archived boards must survive a failed reload"
    );
}

#[test]
fn test_a_failed_scoped_archived_card_read_rolls_back_to_the_previous_model() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Column".to_string(), None)
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Card".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.archive_card(card.id).unwrap();

    app.selection.active_board_id = Some(board.id);
    app.mode = AppMode::ArchivedCardsView;
    app.reload_model();
    assert_eq!(
        app.model
            .board_archived_cards_state(board.id)
            .loaded()
            .map(|v| v.len()),
        Some(1),
        "fixture sanity: one archived card seeded"
    );

    let failing = CountingBackend::wrap_failing(app.ctx.backend(), "list_archived_cards_by_board");
    app.ctx.replace_backend(failing);
    app.reload_model();

    assert_error_banner(&app);
    assert_eq!(
        app.model
            .board_archived_cards_state(board.id)
            .loaded()
            .map(|v| v.len()),
        Some(1),
        "the previous archived cards must survive a failed reload"
    );
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
