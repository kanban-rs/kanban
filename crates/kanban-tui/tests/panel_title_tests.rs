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
        "Tasks [2] (…)",
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

fn title(
    kind: TasksPanelKind,
    count: PanelCount,
    ended: PanelCount,
    filters: Vec<String>,
) -> TasksPanelTitle {
    TasksPanelTitle {
        kind,
        count,
        ended_sprints: ended,
        filters,
    }
}

#[test]
fn test_format_tasks_panel_title_focused_shows_panel_hotkey_and_count() {
    assert_eq!(
        format_tasks_panel_title(&title(
            TasksPanelKind::FocusedTasks,
            PanelCount::Known(3),
            PanelCount::Known(0),
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
            PanelCount::Known(0),
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
            PanelCount::Known(0),
            vec![]
        )),
        "[ARCHIVED] Tasks [2] (5)"
    );
}

#[test]
fn test_format_tasks_panel_title_archive_uses_bracketed_count() {
    assert_eq!(
        format_tasks_panel_title(&title(
            TasksPanelKind::Archive,
            PanelCount::Known(2),
            PanelCount::Known(0),
            vec![]
        )),
        "Archive [2]"
    );
}

#[test]
fn test_format_tasks_panel_title_appends_filter_suffix() {
    assert_eq!(
        format_tasks_panel_title(&title(
            TasksPanelKind::FocusedTasks,
            PanelCount::Known(0),
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
    let columns_by_parent = std::collections::HashMap::from([(board.id, columns.clone())]);
    let cards_by_parent: std::collections::HashMap<uuid::Uuid, LoadState<Vec<Card>>> =
        match &columns {
            LoadState::Loaded(cols) => cols.iter().map(|c| (c.id, cards.clone())).collect(),
            _ => std::collections::HashMap::new(),
        };
    let sprints_by_parent = std::collections::HashMap::from([(board.id, sprints.clone())]);
    let resolved = Resolved {
        boards: Collection {
            all: LoadState::Loaded(vec![board.clone()]),
            ..Default::default()
        },
        cards: Collection {
            by_parent: cards_by_parent,
            ..Default::default()
        },
        columns: Collection {
            by_parent: columns_by_parent,
            ..Default::default()
        },
        sprints: Collection {
            by_parent: sprints_by_parent,
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

fn sprint_at(
    board_id: uuid::Uuid,
    n: u32,
    status: kanban_domain::SprintStatus,
    end_date: Option<chrono::DateTime<chrono::Utc>>,
) -> Sprint {
    Sprint {
        status,
        end_date,
        ..Sprint::new(board_id, n, None, None::<String>)
    }
}

fn ended_sprint(board: &Board) -> Sprint {
    sprint_at(
        board.id,
        1,
        kanban_domain::SprintStatus::Active,
        Some(chrono::Utc::now() - chrono::Duration::days(1)),
    )
}

#[test]
fn test_a_board_with_no_ended_sprints_titles_the_panel_exactly_as_before() {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    let now = chrono::Utc::now();
    let sprints = vec![
        sprint_at(board.id, 1, kanban_domain::SprintStatus::Planning, None),
        sprint_at(
            board.id,
            2,
            kanban_domain::SprintStatus::Completed,
            Some(now - chrono::Duration::days(1)),
        ),
        sprint_at(
            board.id,
            3,
            kanban_domain::SprintStatus::Active,
            Some(now + chrono::Duration::days(1)),
        ),
    ];
    seed_model_states(
        &mut app,
        &board,
        LoadState::Loaded(vec![]),
        LoadState::Loaded(vec![Column::new(board.id, "Todo", 0)]),
        LoadState::Loaded(sprints),
    );

    assert_eq!(tasks_panel_title(&app, false), "Tasks [2] (0)");
}

#[test]
fn test_a_not_loaded_sprint_tier_shows_no_ended_count_rather_than_zero() {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    seed_model_states(
        &mut app,
        &board,
        LoadState::Loaded(vec![]),
        LoadState::Loaded(vec![Column::new(board.id, "Todo", 0)]),
        LoadState::NotLoaded,
    );

    let rendered = tasks_panel_title(&app, false);
    assert!(!rendered.contains("ended"));
}

#[test]
fn test_a_failed_sprint_tier_shows_no_ended_count_rather_than_zero() {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    seed_model_states(
        &mut app,
        &board,
        LoadState::Loaded(vec![]),
        LoadState::Loaded(vec![Column::new(board.id, "Todo", 0)]),
        LoadState::Failed(Arc::new(KanbanError::unsupported("boom"))),
    );

    let rendered = tasks_panel_title(&app, false);
    assert!(!rendered.contains("ended"));
}

#[test]
fn test_a_not_loaded_card_tier_titles_the_panel_without_a_count() {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    seed_model_states(
        &mut app,
        &board,
        LoadState::NotLoaded,
        LoadState::Loaded(vec![Column::new(board.id, "Todo", 0)]),
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
    let column = Column::new(board.id, "Todo", 0);
    seed_model_states(
        &mut app_not_loaded,
        &board,
        LoadState::NotLoaded,
        LoadState::Loaded(vec![column.clone()]),
        LoadState::Loaded(vec![]),
    );
    let not_loaded_rendered = tasks_panel_title(&app_not_loaded, false);

    let mut app_failed = App::test_default();
    seed_model_states(
        &mut app_failed,
        &board,
        boom_cards(),
        LoadState::Loaded(vec![column]),
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
fn test_a_board_with_one_ended_sprint_appends_a_singular_count_to_the_title() {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    seed_model_states(
        &mut app,
        &board,
        LoadState::Loaded(vec![]),
        LoadState::Loaded(vec![Column::new(board.id, "Todo", 0)]),
        LoadState::Loaded(vec![ended_sprint(&board)]),
    );

    assert_eq!(
        tasks_panel_title(&app, false),
        "Tasks [2] (0) - 1 ended sprint"
    );
}

#[test]
fn test_a_board_with_several_ended_sprints_pluralises_the_count() {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    let now = chrono::Utc::now();
    let sprints = vec![
        sprint_at(
            board.id,
            1,
            kanban_domain::SprintStatus::Active,
            Some(now - chrono::Duration::days(1)),
        ),
        sprint_at(
            board.id,
            2,
            kanban_domain::SprintStatus::Active,
            Some(now - chrono::Duration::days(2)),
        ),
    ];
    seed_model_states(
        &mut app,
        &board,
        LoadState::Loaded(vec![]),
        LoadState::Loaded(vec![Column::new(board.id, "Todo", 0)]),
        LoadState::Loaded(sprints),
    );

    assert_eq!(
        tasks_panel_title(&app, false),
        "Tasks [2] (0) - 2 ended sprints"
    );
}

#[test]
fn test_the_ended_sprint_count_comes_after_the_filter_suffix() {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    app.filter.hide_assigned_cards = true;
    seed_model_states(
        &mut app,
        &board,
        LoadState::Loaded(vec![]),
        LoadState::Loaded(vec![Column::new(board.id, "Todo", 0)]),
        LoadState::Loaded(vec![ended_sprint(&board)]),
    );

    assert_eq!(
        tasks_panel_title(&app, true),
        "Tasks [2] (0) - Unassigned Cards - 1 ended sprint"
    );
}

#[test]
fn test_an_archived_cards_view_title_still_reports_the_ended_sprint_count() {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    seed_model_states(
        &mut app,
        &board,
        LoadState::Loaded(vec![]),
        LoadState::Loaded(vec![Column::new(board.id, "Todo", 0)]),
        LoadState::Loaded(vec![ended_sprint(&board)]),
    );
    app.mode = AppMode::ArchivedCardsView;

    let rendered = tasks_panel_title(&app, true);
    assert!(rendered.ends_with(" - 1 ended sprint"));
    assert!(rendered.starts_with("Archive"));
}

#[test]
fn test_the_ended_sprint_count_is_styled_like_the_board_detail_ended_marker() {
    let line = kanban_tui::ui::format_tasks_panel_title_line(&title(
        TasksPanelKind::FocusedTasks,
        PanelCount::Known(3),
        PanelCount::Known(1),
        vec![],
    ));
    let last_span = line.spans.last().unwrap();
    assert_eq!(last_span.style, kanban_tui::theme::ended_marker());
    assert_eq!(last_span.content, " - 1 ended sprint");
    let styled_spans = line
        .spans
        .iter()
        .filter(|s| s.style == kanban_tui::theme::ended_marker())
        .count();
    assert_eq!(styled_spans, 1);
}

#[test]
fn test_the_styled_title_and_the_string_title_carry_identical_text() {
    for ended in [
        PanelCount::Known(0),
        PanelCount::Known(1),
        PanelCount::Known(3),
        PanelCount::NotLoaded,
        PanelCount::Failed,
    ] {
        let t = title(TasksPanelKind::FocusedTasks, PanelCount::Known(2), ended, vec![]);
        assert_eq!(
            kanban_tui::ui::format_tasks_panel_title_line(&t).to_string(),
            format_tasks_panel_title(&t)
        );
    }
}

#[test]
fn test_the_app_native_styled_title_and_its_string_projection_agree() {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    seed_model_states(
        &mut app,
        &board,
        LoadState::Loaded(vec![]),
        LoadState::Loaded(vec![Column::new(board.id, "Todo", 0)]),
        LoadState::Loaded(vec![ended_sprint(&board)]),
    );

    let string_title = tasks_panel_title(&app, true);
    assert_eq!(
        kanban_tui::ui::tasks_panel_title_line(&app, true).to_string(),
        string_title
    );
    assert!(string_title.ends_with(" - 1 ended sprint"));
}

#[test]
fn test_the_ended_sprint_marker_reaches_the_rendered_tasks_panel_title() {
    use ratatui::backend::TestBackend;
    use ratatui::style::{Color, Modifier};
    use ratatui::Terminal;

    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    seed_model_states(
        &mut app,
        &board,
        LoadState::Loaded(vec![]),
        LoadState::Loaded(vec![Column::new(board.id, "Todo", 0)]),
        LoadState::Loaded(vec![ended_sprint(&board)]),
    );

    let backend = TestBackend::new(160, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render(&mut app, frame);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let found = buffer.content().iter().any(|cell| {
        cell.symbol() != " "
            && cell.style().fg == Some(Color::Red)
            && cell.style().add_modifier.contains(Modifier::BOLD)
    });
    assert!(
        found,
        "expected at least one red+bold cell for the ended-sprint marker"
    );
}

#[test]
fn test_a_panel_config_built_from_a_styled_line_keeps_its_span_styles() {
    use kanban_tui::components::PanelConfig;
    use ratatui::text::{Line, Span};

    let cfg = PanelConfig::new(Line::from(vec![
        Span::raw("Tasks"),
        Span::styled(" - 1 ended sprint", kanban_tui::theme::ended_marker()),
    ]));
    assert_eq!(cfg.title_line().spans.len(), 2);
    assert_eq!(cfg.title_line().spans[1].style, kanban_tui::theme::ended_marker());
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
