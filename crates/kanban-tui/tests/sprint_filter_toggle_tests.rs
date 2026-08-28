use kanban_domain::KanbanOperations;
use kanban_tui::app::focus::Focus;
use kanban_tui::components::banner::BannerVariant;
use kanban_tui::App;

#[test]
fn test_toggle_sprint_filter_without_active_sprint_shows_error_banner() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("B".into(), None).unwrap();
    app.ctx
        .create_column(board.id, "Todo".into(), None)
        .unwrap();
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.focus.active = Focus::Cards;
    app.reload_model();
    app.prepare_frame();

    app.handle_toggle_sprint_filter();

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("expected error banner when no active sprint is set");
    assert_eq!(banner.variant, BannerVariant::Error);
    assert!(
        banner.message.contains("No active sprint"),
        "banner should explain why toggling did nothing: {}",
        banner.message,
    );
}
