use kanban_domain::resolved::Collection;
use kanban_domain::{
    Board, Card, Column, CreateCardOptions, DependencyGraph, KanbanError, KanbanOperations,
    LoadState, Resolved, Sprint,
};
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::AppMode;
use kanban_tui::ui::{
    filter_title_suffix, format_filter_title_suffix, format_tasks_panel_title, tasks_panel_title,
};
use kanban_tui::App;
use kanban_view::panel_titles::{PanelCount, TasksPanelKind, TasksPanelTitle};
use std::sync::Arc;

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

fn title(kind: TasksPanelKind, count: PanelCount, filters: Vec<String>) -> TasksPanelTitle {
    TasksPanelTitle {
        kind,
        count,
        filters,
    }
}

#[test]
fn test_format_tasks_panel_title_focused_shows_panel_hotkey_and_count() {
    assert_eq!(
        format_tasks_panel_title(&title(
            TasksPanelKind::FocusedTasks,
            PanelCount::Known(3),
            vec![]
        )),
        "Tasks [2] (3)"
    );
}

#[test]
fn test_format_tasks_panel_title_unfocused_omits_hotkey_and_count() {
    assert_eq!(
        format_tasks_panel_title(&title(
            TasksPanelKind::UnfocusedTasks,
            PanelCount::Known(3),
            vec![]
        )),
        "Tasks"
    );
}

#[test]
fn test_format_tasks_panel_title_archived_board_prefixes_marker() {
    assert_eq!(
        format_tasks_panel_title(&title(
            TasksPanelKind::ArchivedBoardTasks,
            PanelCount::Known(5),
            vec![]
        )),
        "[ARCHIVED] Tasks [2] (5)"
    );
}

#[test]
fn test_format_tasks_panel_title_archive_uses_bracketed_count() {
    assert_eq!(
        format_tasks_panel_title(&title(TasksPanelKind::Archive, PanelCount::Known(2), vec![])),
        "Archive [2]"
    );
}

#[test]
fn test_format_tasks_panel_title_appends_filter_suffix() {
    assert_eq!(
        format_tasks_panel_title(&title(
            TasksPanelKind::FocusedTasks,
            PanelCount::Known(0),
            vec!["Unassigned Cards".to_string()]
        )),
        "Tasks [2] (0) - Unassigned Cards"
    );
}

fn seed_model_states(
    app: &mut App,
    board: &Board,
    cards: LoadState<Vec<Card>>,
    columns: LoadState<Vec<Column>>,
    sprints: LoadState<Vec<Sprint>>,
) {
    let resolved = Resolved {
        boards: Collection {
            all: LoadState::Loaded(vec![board.clone()]),
            ..Default::default()
        },
        cards: Collection {
            all: cards,
            ..Default::default()
        },
        columns: Collection {
            all: columns,
            ..Default::default()
        },
        sprints: Collection {
            all: sprints,
            ..Default::default()
        },
        graph: LoadState::Loaded(DependencyGraph::default()),
        ..Default::default()
    };
    let _ = app.model.apply_resolved(resolved);
    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Cards;
    app.prepare_frame();
}

fn boom_cards() -> LoadState<Vec<Card>> {
    LoadState::Failed(Arc::new(KanbanError::unsupported("boom")))
}

#[test]
fn test_a_not_loaded_card_tier_titles_the_panel_without_a_count() {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    seed_model_states(
        &mut app,
        &board,
        LoadState::NotLoaded,
        LoadState::Loaded(vec![]),
        LoadState::Loaded(vec![]),
    );

    let rendered = tasks_panel_title(&app, false);
    assert!(
        !rendered
            .split('(')
            .nth(1)
            .is_some_and(|tail| tail.chars().next().is_some_and(|c| c.is_ascii_digit())),
        "a NotLoaded card tier must not print a confident count: {rendered}"
    );
}

#[test]
fn test_a_loaded_empty_card_tier_still_titles_the_panel_with_zero() {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    seed_model_states(
        &mut app,
        &board,
        LoadState::Loaded(vec![]),
        LoadState::Loaded(vec![Column::new(board.id, "Todo", 0)]),
        LoadState::Loaded(vec![]),
    );

    assert_eq!(tasks_panel_title(&app, false), "Tasks [2] (0)");
}

#[test]
fn test_a_failed_card_tier_titles_the_panel_distinctly_from_not_loaded() {
    let mut app_not_loaded = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    seed_model_states(
        &mut app_not_loaded,
        &board,
        LoadState::NotLoaded,
        LoadState::Loaded(vec![]),
        LoadState::Loaded(vec![]),
    );
    let not_loaded_rendered = tasks_panel_title(&app_not_loaded, false);

    let mut app_failed = App::test_default();
    seed_model_states(
        &mut app_failed,
        &board,
        boom_cards(),
        LoadState::Loaded(vec![]),
        LoadState::Loaded(vec![]),
    );
    let failed_rendered = tasks_panel_title(&app_failed, false);

    assert_ne!(
        not_loaded_rendered, failed_rendered,
        "a Failed card tier must render distinctly from a NotLoaded one"
    );
}

#[test]
fn test_the_archived_panel_title_count_is_state_aware_too() {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    seed_model_states(
        &mut app,
        &board,
        LoadState::NotLoaded,
        LoadState::Loaded(vec![]),
        LoadState::Loaded(vec![]),
    );
    app.mode = AppMode::ArchivedCardsView;

    let rendered = tasks_panel_title(&app, false);
    assert!(
        !rendered
            .split('[')
            .nth(1)
            .is_some_and(|tail| tail.chars().next().is_some_and(|c| c.is_ascii_digit())),
        "the Archive panel's bracketed count must not show a digit when the card tier is not loaded: {rendered}"
    );
}

#[test]
fn test_tasks_panel_title_with_loaded_cards_but_not_loaded_columns_reports_not_loaded() {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    let column = Column::new(board.id, "Todo", 0);
    let card = Card::new(board.id, column.id, "Card", 0);
    seed_model_states(
        &mut app,
        &board,
        LoadState::Loaded(vec![card]),
        LoadState::NotLoaded,
        LoadState::Loaded(vec![]),
    );

    let rendered = tasks_panel_title(&app, false);
    assert!(
        !rendered
            .split('(')
            .nth(1)
            .is_some_and(|tail| tail.chars().next().is_some_and(|c| c.is_ascii_digit())),
        "cards Loaded but columns NotLoaded must still report NotLoaded, not a stale digit: {rendered}"
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
