//! Pins the render source for the tasks panel and its counted title off the
//! board-scoped `Model`/`Controller` tiers for cards and archived cards,
//! rather than the flat, workspace-wide ones. Columns and sprints stay on
//! the flat tiers in this card (`ViewScope` does not request them by board
//! outside CardDetail/BoardDetail/SprintDetail), so this file wraps only
//! the card-side flat reads as failing.

mod helpers;

use helpers::CountingBackend;
use kanban_domain::{CreateCardOptions, KanbanOperations};
use kanban_tui::app::focus::Focus;
use kanban_tui::App;

#[test]
fn test_tasks_panel_title_gates_on_scoped_tiers() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    app.ctx
        .create_card(
            board.id,
            column.id,
            "Card".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();

    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Cards;
    app.reload_model();
    app.prepare_frame();

    let title_before = kanban_tui::ui::tasks_panel_title(&app, false);
    assert!(
        title_before.contains('1'),
        "expected a numeric count before the backend degrades: {title_before}"
    );

    let failing = CountingBackend::wrap_failing_all(
        app.ctx.backend(),
        &["list_all_cards", "list_archived_cards"],
    );
    app.ctx.replace_backend(failing);

    app.reload_model();
    app.prepare_frame();

    let title_after = kanban_tui::ui::tasks_panel_title(&app, false);
    assert!(
        title_after.contains('1'),
        "expected the scoped card tier to still render a numeric count: {title_after}"
    );
    assert!(
        !title_after.contains('!'),
        "expected no failure marker once the scoped tier is being served: {title_after}"
    );
    assert!(
        app.ui_state.banner.is_none(),
        "expected no rollback banner when only the flat card tiers decline: {title_after}"
    );
}

#[test]
fn test_surface_load_failures_surfaces_a_scoped_card_tier_failure() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    app.ctx
        .create_card(
            board.id,
            column.id,
            "Card".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();

    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Cards;
    app.reload_model();
    app.prepare_frame();

    let cards_before = app.displayed_cards().loaded().copied().unwrap_or(&[]).len();
    assert_eq!(cards_before, 1, "fixture sanity: one live card seeded");

    let failing = CountingBackend::wrap_failing(app.ctx.backend(), "list_cards_by_column");
    app.ctx.replace_backend(failing);

    app.reload_model();

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("expected surface_load_failures to raise a banner on a scoped card failure");
    assert!(banner.message.contains("Failed to load from store"));

    let cards_after = app.displayed_cards().loaded().copied().unwrap_or(&[]).len();
    assert_eq!(
        cards_after, 1,
        "expected the rollback to keep the previous model's card intact"
    );
}
