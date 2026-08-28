use kanban_domain::{CreateCardOptions, KanbanOperations};
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::AppMode;
use kanban_tui::ui::{
    filter_title_suffix, format_filter_title_suffix, format_tasks_panel_title, tasks_panel_title,
};
use kanban_tui::App;
use kanban_view::panel_titles::{TasksPanelKind, TasksPanelTitle};

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
    app.reload_model();
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
    app.reload_model();
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
    app.reload_model();
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
    app.reload_model();
    app.prepare_frame();

    assert_eq!(app.active_board(), None);
    assert_eq!(
        filter_title_suffix(&app),
        None,
        "a sprint filter can't be named without a board to resolve sprint names against"
    );
}

fn title(kind: TasksPanelKind, count: usize, filters: Vec<String>) -> TasksPanelTitle {
    TasksPanelTitle {
        kind,
        count,
        filters,
    }
}

#[test]
fn test_format_tasks_panel_title_focused_shows_panel_hotkey_and_count() {
    assert_eq!(
        format_tasks_panel_title(&title(TasksPanelKind::FocusedTasks, 3, vec![])),
        "Tasks [2] (3)"
    );
}

#[test]
fn test_format_tasks_panel_title_unfocused_omits_hotkey_and_count() {
    assert_eq!(
        format_tasks_panel_title(&title(TasksPanelKind::UnfocusedTasks, 3, vec![])),
        "Tasks"
    );
}

#[test]
fn test_format_tasks_panel_title_archived_board_prefixes_marker() {
    assert_eq!(
        format_tasks_panel_title(&title(TasksPanelKind::ArchivedBoardTasks, 5, vec![])),
        "[ARCHIVED] Tasks [2] (5)"
    );
}

#[test]
fn test_format_tasks_panel_title_archive_uses_bracketed_count() {
    assert_eq!(
        format_tasks_panel_title(&title(TasksPanelKind::Archive, 2, vec![])),
        "Archive [2]"
    );
}

#[test]
fn test_format_tasks_panel_title_appends_filter_suffix() {
    assert_eq!(
        format_tasks_panel_title(&title(
            TasksPanelKind::FocusedTasks,
            0,
            vec!["Unassigned Cards".to_string()]
        )),
        "Tasks [2] (0) - Unassigned Cards"
    );
}

#[test]
fn test_format_filter_title_suffix_empty_parts_returns_none() {
    assert_eq!(format_filter_title_suffix(&[]), None);
}

#[test]
fn test_format_filter_title_suffix_joins_parts_with_plus() {
    assert_eq!(
        format_filter_title_suffix(&[
            "Unassigned Cards".to_string(),
            "sprint-1/Sprint A".to_string()
        ]),
        Some(" - Unassigned Cards + sprint-1/Sprint A".to_string())
    );
}
