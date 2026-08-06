use kanban_domain::{CreateCardOptions, KanbanOperations};
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::AppMode;
use kanban_tui::ui::{filter_title_suffix, tasks_panel_title};
use kanban_tui::App;

#[test]
fn test_build_tasks_panel_title_cards_focus_with_cards() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("B".to_string(), None).unwrap();
    let col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    for i in 0..3 {
        app.ctx
            .create_card(
                board.id,
                col.id,
                format!("Card{i}"),
                CreateCardOptions::default(),
            )
            .unwrap();
    }
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.focus.active = Focus::Cards;
    app.prepare_frame();
    assert_eq!(
        tasks_panel_title(&app, false),
        "Tasks [2] (3)",
        "should show keyboard shortcut [2] and actual card count"
    );
}

#[test]
fn test_build_tasks_panel_title_archived_view_with_cards() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("B".to_string(), None).unwrap();
    let col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    for i in 0..2 {
        let card = app
            .ctx
            .create_card(
                board.id,
                col.id,
                format!("Card{i}"),
                CreateCardOptions::default(),
            )
            .unwrap();
        app.ctx.archive_card(card.id).unwrap();
    }
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.mode = AppMode::ArchivedCardsView;
    app.prepare_frame();
    assert_eq!(
        tasks_panel_title(&app, false),
        "Archive [2]",
        "should show archived card count"
    );
}

#[test]
fn test_tasks_panel_title_with_no_active_board_omits_sprint_filter_suffix() {
    let mut app = App::test_default();
    app.focus.active = Focus::Cards;
    app.filter
        .active_sprint_filters
        .insert(uuid::Uuid::new_v4());
    app.prepare_frame();

    assert_eq!(
        app.active_board(),
        None,
        "no board has been created or selected"
    );
    assert_eq!(
        tasks_panel_title(&app, true),
        "Tasks [2] (0)",
        "with no active board, the sprint-filter suffix must be omitted rather than panic"
    );
}

#[test]
fn test_filter_title_suffix_with_no_active_board_omits_sprint_filter_suffix() {
    let mut app = App::test_default();
    app.filter
        .active_sprint_filters
        .insert(uuid::Uuid::new_v4());
    app.prepare_frame();

    assert_eq!(app.active_board(), None);
    assert_eq!(
        filter_title_suffix(&app),
        None,
        "a sprint filter can't be named without a board to resolve sprint names against"
    );
}
