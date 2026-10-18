mod helpers;

use kanban_domain::resolved::Collection;
use kanban_domain::{
    Board, Card, Column, DependencyGraph, DerivedProjections, LoadState, Resolved, Sprint,
};
use kanban_tui::app::Focus;
use kanban_tui::render_strategy::{RenderStrategy, SinglePanelRenderer};
use kanban_tui::App;
use kanban_view::view_strategy::ViewRefreshContext;
use std::collections::HashMap;

fn seed_board_with_scoped_sprints(
    build_scoped: impl FnOnce(uuid::Uuid) -> Vec<Sprint>,
) -> (App, Board) {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    let scoped = build_scoped(board.id);

    let resolved = Resolved {
        boards: Collection {
            all: LoadState::Loaded(vec![board.clone()]),
            ..Default::default()
        },
        sprints: Collection {
            by_parent: HashMap::from([(board.id, LoadState::Loaded(scoped))]),
            ..Default::default()
        },
        graph: LoadState::Loaded(DependencyGraph::default()),
        ..Default::default()
    };
    let changed = app.model.apply_resolved(resolved);
    app.controller.resync(&app.model, changed);
    app.selection.active_board_id = Some(board.id);
    (app, board)
}

fn open_filter_dialog(app: &mut App) {
    app.focus.active = Focus::Cards;
    app.handle_open_filter_dialog();
}

#[test]
fn test_filter_dialog_sprint_toggle_targets_rendered_sprint() {
    let s_b_id = std::cell::Cell::new(uuid::Uuid::nil());
    let (mut app, _board) = seed_board_with_scoped_sprints(|board_id| {
        let s_b = Sprint::new(board_id, 2, None, None::<String>);
        s_b_id.set(s_b.id);
        vec![s_b]
    });
    let s_b_id = s_b_id.get();

    open_filter_dialog(&mut app);
    let dialog_state = app
        .filter
        .dialog_state
        .as_mut()
        .expect("filter dialog must be open");
    dialog_state.item_selection = 1;

    app.handle_filter_options_popup(crossterm::event::KeyCode::Char(' '));

    assert!(app.filter.active_sprint_filters.contains(&s_b_id));
}

#[test]
fn test_up_from_date_range_enters_sprints_at_last_item() {
    let (mut app, _board) = seed_board_with_scoped_sprints(|board_id| {
        vec![
            Sprint::new(board_id, 1, None, None::<String>),
            Sprint::new(board_id, 2, None, None::<String>),
        ]
    });

    open_filter_dialog(&mut app);

    // Navigate DOWN through all sprint items to reach DateRange (section 1).
    // 2 sprints => 3 items (unassigned + 2), need 3 'j' presses to advance past them.
    app.handle_filter_options_popup(crossterm::event::KeyCode::Char('j')); // item 0 → 1
    app.handle_filter_options_popup(crossterm::event::KeyCode::Char('j')); // item 1 → 2
    app.handle_filter_options_popup(crossterm::event::KeyCode::Char('j')); // item 2 → DateRange
    {
        let ds = app.filter.dialog_state.as_ref().unwrap();
        assert_eq!(
            ds.current_section,
            kanban_view::filters::FilterDialogSection::DateRange
        );
    }

    // Navigate UP from DateRange back to Sprints — should land on the last item
    app.handle_filter_options_popup(crossterm::event::KeyCode::Char('k'));
    {
        let ds = app.filter.dialog_state.as_ref().unwrap();
        assert_eq!(
            ds.current_section,
            kanban_view::filters::FilterDialogSection::Sprints
        );
        // 2 sprints => total_items = 3 (unassigned + 2), last = 2
        assert_eq!(ds.item_selection, 2);
    }
}

#[test]
fn test_down_from_tags_wraps_to_sprints_at_first_item() {
    let (mut app, _board) = seed_board_with_scoped_sprints(|board_id| {
        vec![
            Sprint::new(board_id, 1, None, None::<String>),
            Sprint::new(board_id, 2, None, None::<String>),
        ]
    });

    open_filter_dialog(&mut app);

    // Navigate DOWN through all sprint items, then DateRange, then to Tags
    app.handle_filter_options_popup(crossterm::event::KeyCode::Char('j')); // item 0 → 1
    app.handle_filter_options_popup(crossterm::event::KeyCode::Char('j')); // item 1 → 2
    app.handle_filter_options_popup(crossterm::event::KeyCode::Char('j')); // item 2 → DateRange
    app.handle_filter_options_popup(crossterm::event::KeyCode::Char('j')); // DateRange → Tags
    {
        let ds = app.filter.dialog_state.as_ref().unwrap();
        assert_eq!(
            ds.current_section,
            kanban_view::filters::FilterDialogSection::Tags
        );
    }

    // Navigate DOWN from Tags (wrap) to Sprints — should land on the first item
    app.handle_filter_options_popup(crossterm::event::KeyCode::Char('j'));
    {
        let ds = app.filter.dialog_state.as_ref().unwrap();
        assert_eq!(
            ds.current_section,
            kanban_view::filters::FilterDialogSection::Sprints
        );
        assert_eq!(ds.item_selection, 0);
    }
}

#[test]
fn test_filter_dialog_sprint_nav_counts_the_rendered_rows() {
    let (mut app, _board) = seed_board_with_scoped_sprints(|board_id| {
        vec![Sprint::new(board_id, 2, None, None::<String>)]
    });

    open_filter_dialog(&mut app);
    {
        let dialog_state = app
            .filter
            .dialog_state
            .as_mut()
            .expect("filter dialog must be open");
        dialog_state.item_selection = 1;
    }

    app.handle_filter_options_popup(crossterm::event::KeyCode::Char('j'));

    let dialog_state = app
        .filter
        .dialog_state
        .as_ref()
        .expect("filter dialog stays open on next-section advance");
    assert_ne!(
        dialog_state.current_section,
        kanban_view::filters::FilterDialogSection::Sprints
    );
    assert_eq!(dialog_state.item_selection, 0);
}

#[test]
fn test_task_row_sprint_suffix_not_loaded_renders_marker_not_blank() {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    let column = Column::new(board.id, "Backlog", 0);
    let mut card = Card::new(board.id, column.id, "Task", 0);
    card.sprint_id = Some(uuid::Uuid::new_v4());

    let resolved = Resolved {
        boards: Collection {
            all: LoadState::Loaded(vec![board.clone()]),
            ..Default::default()
        },
        cards: Collection {
            by_parent: HashMap::from([(column.id, LoadState::Loaded(vec![card.clone()]))]),
            ..Default::default()
        },
        columns: Collection {
            by_parent: HashMap::from([(board.id, LoadState::Loaded(vec![column.clone()]))]),
            ..Default::default()
        },
        graph: LoadState::Loaded(DependencyGraph::default()),
        ..Default::default()
    };
    let changed = app.model.apply_resolved(resolved);
    app.controller.resync(&app.model, changed);
    app.selection.active_board_id = Some(board.id);
    app.switch_view_strategy(kanban_domain::TaskListView::ColumnView);
    app.view.strategy.refresh_task_lists(&ViewRefreshContext {
        board: &board,
        all_cards: &[card],
        all_columns: &[column],
        all_sprints: &[],
        active_sprint_filters: Default::default(),
        hide_assigned_cards: false,
        search_query: None,
    });

    let rendered = helpers::render_widget_to_string(80, 20, |f| {
        SinglePanelRenderer::grouped().render(&app, f, f.area());
    });

    assert!(
        rendered.contains("(\u{2026})"),
        "expected the not-loaded sprint marker in:\n{rendered}"
    );
}

#[test]
fn test_task_row_sprint_suffix_loaded_renders_the_sprint_name() {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    let column = Column::new(board.id, "Backlog", 0);
    let mut sprint = Sprint::new(board.id, 1, None, None::<String>);
    sprint.prefix = Some("SPR".to_string());
    let mut card = Card::new(board.id, column.id, "Task", 0);
    card.sprint_id = Some(sprint.id);

    let resolved = Resolved {
        boards: Collection {
            all: LoadState::Loaded(vec![board.clone()]),
            ..Default::default()
        },
        cards: Collection {
            by_parent: HashMap::from([(column.id, LoadState::Loaded(vec![card.clone()]))]),
            ..Default::default()
        },
        columns: Collection {
            by_parent: HashMap::from([(board.id, LoadState::Loaded(vec![column.clone()]))]),
            ..Default::default()
        },
        sprints: Collection {
            by_parent: HashMap::from([(board.id, LoadState::Loaded(vec![sprint.clone()]))]),
            ..Default::default()
        },
        graph: LoadState::Loaded(DependencyGraph::default()),
        ..Default::default()
    };
    let changed = app.model.apply_resolved(resolved);
    app.controller.resync(&app.model, changed);
    app.selection.active_board_id = Some(board.id);
    app.switch_view_strategy(kanban_domain::TaskListView::ColumnView);
    app.view.strategy.refresh_task_lists(&ViewRefreshContext {
        board: &board,
        all_cards: &[card],
        all_columns: &[column],
        all_sprints: std::slice::from_ref(&sprint),
        active_sprint_filters: Default::default(),
        hide_assigned_cards: false,
        search_query: None,
    });

    let rendered = helpers::render_widget_to_string(80, 20, |f| {
        SinglePanelRenderer::grouped().render(&app, f, f.area());
    });

    assert!(
        !rendered.contains("(\u{2026})"),
        "must not show the not-loaded marker once the scoped tier resolves:\n{rendered}"
    );
    assert!(
        rendered.contains(&sprint.formatted_name(&board, None)),
        "expected the loaded sprint's name in:\n{rendered}"
    );
}
