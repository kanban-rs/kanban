mod helpers;

use kanban_domain::resolved::Collection;
use kanban_domain::{
    Board, Card, Column, DependencyGraph, DerivedProjections, KanbanError, LoadState, Resolved,
    Sprint,
};
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::App;
use std::collections::HashMap;
use std::sync::Arc;

fn base_resolved(board: &Board) -> Resolved {
    Resolved {
        boards: Collection {
            all: LoadState::Loaded(vec![board.clone()]),
            ..Default::default()
        },
        graph: LoadState::Loaded(DependencyGraph::default()),
        ..Default::default()
    }
}

fn app_with_board_and_column_state(state: LoadState<Vec<Column>>) -> (App, Board) {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    let mut resolved = base_resolved(&board);
    resolved.columns = Collection {
        by_parent: HashMap::from([(board.id, state)]),
        ..Default::default()
    };
    resolved.sprints = Collection {
        by_parent: HashMap::from([(board.id, LoadState::Loaded(vec![]))]),
        ..Default::default()
    };
    let _ = app.model.apply_resolved(resolved);
    app.selection.active_board_id = Some(board.id);
    (app, board)
}

fn app_with_board_and_sprint_state(state: LoadState<Vec<Sprint>>) -> (App, Board) {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    let mut resolved = base_resolved(&board);
    resolved.sprints = Collection {
        by_parent: HashMap::from([(board.id, state)]),
        ..Default::default()
    };
    let _ = app.model.apply_resolved(resolved);
    app.selection.active_board_id = Some(board.id);
    (app, board)
}

fn app_with_card_and_sprint_state(state: LoadState<Vec<Sprint>>) -> (App, Board, Card) {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    let column = Column::new(board.id, "Backlog", 0);
    let card = Card::new(board.id, column.id, "Task", 0);
    let mut resolved = base_resolved(&board);
    resolved.cards = Collection {
        by_parent: HashMap::from([(column.id, LoadState::Loaded(vec![card.clone()]))]),
        ..Default::default()
    };
    resolved.columns = Collection {
        by_parent: HashMap::from([(board.id, LoadState::Loaded(vec![column.clone()]))]),
        ..Default::default()
    };
    resolved.sprints = Collection {
        by_parent: HashMap::from([(board.id, state)]),
        ..Default::default()
    };
    let _ = app.model.apply_resolved(resolved);
    app.selection.active_board_id = Some(board.id);
    app.selection.active_card_id = Some(card.id);
    (app, board, card)
}

fn app_with_card_and_graph_state(state: LoadState<DependencyGraph>) -> (App, Board, Card) {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    let column = Column::new(board.id, "Backlog", 0);
    let card = Card::new(board.id, column.id, "Task", 0);
    let mut resolved = base_resolved(&board);
    resolved.cards = Collection {
        by_parent: HashMap::from([(column.id, LoadState::Loaded(vec![card.clone()]))]),
        ..Default::default()
    };
    resolved.columns = Collection {
        by_parent: HashMap::from([(board.id, LoadState::Loaded(vec![column.clone()]))]),
        ..Default::default()
    };
    resolved.sprints = Collection {
        by_parent: HashMap::from([(board.id, LoadState::Loaded(vec![]))]),
        ..Default::default()
    };
    resolved.graph = state;
    let _ = app.model.apply_resolved(resolved);
    app.selection.active_board_id = Some(board.id);
    app.selection.active_card_id = Some(card.id);
    (app, board, card)
}

fn app_with_asymmetric_column_tier(board_columns_state: LoadState<Vec<Column>>) -> App {
    let mut app = App::test_default();
    let board = Board::new("TestBoard", None::<String>);
    let column = Column::new(board.id, "Backlog", 0);
    let mut resolved = base_resolved(&board);
    resolved.columns = Collection {
        by_parent: HashMap::from([(board.id, board_columns_state)]),
        ..Default::default()
    };
    resolved.sprints = Collection {
        by_parent: HashMap::from([(board.id, LoadState::Loaded(vec![]))]),
        ..Default::default()
    };
    let changed = app.model.apply_resolved(resolved);
    app.controller.resync(&app.model, changed);
    app.selection.active_board_id = Some(board.id);
    app.switch_view_strategy(kanban_domain::TaskListView::ColumnView);

    // `prepare_frame`'s all-scoped-tiers-Loaded gate would decline for a
    // non-Loaded `board_columns_state`, so drive `refresh_task_lists`
    // directly to build the task-list skeleton; the render's own
    // `board_columns_state` name lookup is what's under test.
    let ctx = kanban_view::view_strategy::ViewRefreshContext {
        board: &board,
        all_cards: &[],
        all_columns: std::slice::from_ref(&column),
        all_sprints: &[],
        active_sprint_filters: Default::default(),
        hide_assigned_cards: false,
        search_query: None,
    };
    app.view.strategy.refresh_task_lists(&ctx);
    app
}

fn boom() -> LoadState<Vec<Column>> {
    LoadState::Failed(Arc::new(KanbanError::unsupported("boom")))
}

fn boom_sprints() -> LoadState<Vec<Sprint>> {
    LoadState::Failed(Arc::new(KanbanError::unsupported("boom")))
}

struct NoActiveTaskList;

impl kanban_view::view_strategy::ViewStrategy for NoActiveTaskList {
    fn get_active_task_list(&self) -> Option<&kanban_view::card_list::CardList> {
        None
    }
    fn get_active_task_list_mut(&mut self) -> Option<&mut kanban_view::card_list::CardList> {
        None
    }
    fn get_all_task_lists(&self) -> Vec<&kanban_view::card_list::CardList> {
        vec![]
    }
    fn navigate_left(&mut self, _select_last: bool) -> bool {
        false
    }
    fn navigate_right(&mut self, _select_last: bool) -> bool {
        false
    }
    fn refresh_task_lists(&mut self, _ctx: &kanban_view::view_strategy::ViewRefreshContext) {}
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn test_a_not_loaded_column_tier_does_not_render_the_no_columns_message() {
    let (mut app, _board) = app_with_board_and_column_state(LoadState::NotLoaded);
    app.view.strategy = Box::new(NoActiveTaskList);
    let output = helpers::render_widget_to_string(80, 20, |frame| {
        use kanban_tui::render_strategy::{RenderStrategy, SinglePanelRenderer};
        SinglePanelRenderer::grouped().render(&app, frame, frame.area());
    });
    assert!(!output.contains("No columns yet"));
    assert!(output.contains("Columns not loaded yet"));
}

#[test]
fn test_a_loaded_but_empty_column_tier_still_renders_the_no_columns_message() {
    let (mut app, _board) = app_with_board_and_column_state(LoadState::Loaded(vec![]));
    app.view.strategy = Box::new(NoActiveTaskList);
    let output = helpers::render_widget_to_string(80, 20, |frame| {
        use kanban_tui::render_strategy::{RenderStrategy, SinglePanelRenderer};
        SinglePanelRenderer::grouped().render(&app, frame, frame.area());
    });
    assert!(output.contains("No columns yet. Add columns in board settings."));
}

#[test]
fn test_a_failed_column_tier_renders_the_error_inline() {
    let (mut app, _board) = app_with_board_and_column_state(boom());
    app.view.strategy = Box::new(NoActiveTaskList);
    let output = helpers::render_widget_to_string(80, 20, |frame| {
        use kanban_tui::render_strategy::{RenderStrategy, SinglePanelRenderer};
        SinglePanelRenderer::grouped().render(&app, frame, frame.area());
    });
    assert!(!output.contains("No columns yet"));
    assert!(output.contains("Columns failed to load"));
    assert!(output.contains("boom"));
}

#[test]
fn test_a_missing_column_tier_renders_distinctly_from_not_loaded() {
    let (mut app, _board) = app_with_board_and_column_state(LoadState::Missing);
    app.view.strategy = Box::new(NoActiveTaskList);
    let output = helpers::render_widget_to_string(80, 20, |frame| {
        use kanban_tui::render_strategy::{RenderStrategy, SinglePanelRenderer};
        SinglePanelRenderer::grouped().render(&app, frame, frame.area());
    });
    assert!(!output.contains("No columns yet"));
    assert!(output.contains("Columns not found"));
    assert!(!output.contains("Columns not loaded yet"));
}

#[test]
fn test_a_not_loaded_sprint_tier_does_not_render_no_sprints_available() {
    use kanban_domain::CardFilters;
    use kanban_view::filters::FilterDialogState;
    let (mut app, _board) = app_with_board_and_sprint_state(LoadState::NotLoaded);
    app.mode = AppMode::Dialog(DialogMode::FilterOptions);
    app.filter.dialog_state = Some(FilterDialogState::new(CardFilters::default()));
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::components::render_filter_options_popup(&app, frame);
    });
    assert!(!output.contains("(no sprints available)"));
    assert!(output.contains("Sprints not loaded yet"));
}

#[test]
fn test_a_loaded_but_empty_sprint_tier_still_renders_no_sprints_available() {
    use kanban_domain::CardFilters;
    use kanban_view::filters::FilterDialogState;
    let (mut app, _board) = app_with_board_and_sprint_state(LoadState::Loaded(vec![]));
    app.mode = AppMode::Dialog(DialogMode::FilterOptions);
    app.filter.dialog_state = Some(FilterDialogState::new(CardFilters::default()));
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::components::render_filter_options_popup(&app, frame);
    });
    assert!(output.contains("(no sprints available)"));
}

#[test]
fn test_render_filter_sprints_section_always_draws_its_panel() {
    use kanban_domain::CardFilters;
    use kanban_view::filters::FilterDialogState;
    for state in [
        LoadState::NotLoaded,
        LoadState::Missing,
        boom_sprints(),
        LoadState::Loaded(vec![]),
    ] {
        let (mut app, _board) = app_with_board_and_sprint_state(state);
        app.mode = AppMode::Dialog(DialogMode::FilterOptions);
        app.filter.dialog_state = Some(FilterDialogState::new(CardFilters::default()));
        let output = helpers::render_widget_to_string(120, 40, |frame| {
            kanban_tui::components::render_filter_options_popup(&app, frame);
        });
        assert!(
            output.contains("Sprints"),
            "the Sprints section border/title must always draw"
        );
    }
}

#[test]
fn test_sprint_detail_with_an_unloaded_sprint_renders_an_unavailable_panel() {
    let mut app = App::test_default();
    app.selection.active_sprint_id = Some(uuid::Uuid::new_v4());
    app.mode = AppMode::SprintDetail;
    let output = helpers::render_widget_to_string(120, 30, |frame| {
        kanban_tui::ui::render(&mut app, frame);
    });
    assert!(output.contains("Sprint"));
    assert!(!output.trim().is_empty());
}

#[test]
fn test_sprint_detail_with_no_sprint_selected_still_draws_nothing() {
    let mut app = App::test_default();
    app.selection.active_sprint_id = None;
    app.mode = AppMode::SprintDetail;
    let output = helpers::render_widget_to_string(120, 30, |frame| {
        kanban_tui::ui::render(&mut app, frame);
    });
    let main_area_lines: String = output.lines().take(27).collect();
    assert!(
        !main_area_lines.contains("Sprint Details"),
        "no sprint selected must draw nothing in the main panel"
    );
}

#[test]
fn test_card_detail_with_an_unloaded_sprint_tier_does_not_render_no_sprint() {
    let (mut app, _board, _card) = app_with_card_and_sprint_state(LoadState::NotLoaded);
    app.mode = AppMode::CardDetail;
    let output = helpers::render_widget_to_string(120, 30, |frame| {
        kanban_tui::ui::render(&mut app, frame);
    });
    assert!(output.contains("Sprints not loaded yet"));
}

#[test]
fn test_card_detail_with_a_loaded_sprint_tier_renders_metadata_normally() {
    let (mut app, _board, _card) = app_with_card_and_sprint_state(LoadState::Loaded(vec![]));
    app.mode = AppMode::CardDetail;
    let output = helpers::render_widget_to_string(120, 30, |frame| {
        kanban_tui::ui::render(&mut app, frame);
    });
    assert!(output.contains("Priority"));
    assert!(!output.contains("Sprints not loaded yet"));
}

#[test]
fn test_board_detail_columns_section_distinguishes_not_loaded_from_empty() {
    let (mut app, _board) = app_with_board_and_column_state(LoadState::NotLoaded);
    app.mode = AppMode::BoardDetail;
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::ui::render(&mut app, frame);
    });
    assert!(!output.contains("No columns yet"));
    assert!(output.contains("Columns not loaded yet"));
}

#[test]
fn test_board_detail_columns_section_still_renders_no_columns_message_when_loaded_empty() {
    let (mut app, _board) = app_with_board_and_column_state(LoadState::Loaded(vec![]));
    app.mode = AppMode::BoardDetail;
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::ui::render(&mut app, frame);
    });
    assert!(output.contains("No columns yet. Press 'n' to create one!"));
}

#[test]
fn test_the_create_card_dialog_sprint_field_distinguishes_not_loaded_from_empty() {
    let (mut app, _board) = app_with_board_and_sprint_state(LoadState::NotLoaded);
    app.mode = AppMode::Dialog(DialogMode::CreateCard);
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::ui::render(&mut app, frame);
    });
    assert!(output.contains("Sprint"));
    assert!(output.contains("not loaded yet"));
}

#[test]
fn test_the_sprint_picker_distinguishes_not_loaded_from_empty() {
    let (mut app, _board) = app_with_board_and_sprint_state(LoadState::NotLoaded);
    app.mode = AppMode::Dialog(DialogMode::AssignMultipleCardsToSprint);
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::ui::render(&mut app, frame);
    });
    assert!(output.contains("Sprint"));
    assert!(output.contains("not loaded yet"));
}

#[test]
fn test_carry_over_sprint_dialog_distinguishes_not_loaded_from_empty() {
    use kanban_tui::components::selection_dialog::CarryOverSprintDialog;
    use kanban_tui::components::SelectionDialog;

    let (mut app, _board) = app_with_board_and_sprint_state(LoadState::NotLoaded);
    let dialog = CarryOverSprintDialog { card_count: 0 };
    assert_eq!(dialog.options_count(&app), 0);

    app.mode = AppMode::Dialog(DialogMode::CarryOverSprint);
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::ui::render(&mut app, frame);
    });
    assert!(output.contains("Sprints not loaded yet"));
}

#[test]
fn test_carry_over_sprint_dialog_still_renders_when_loaded_empty() {
    use kanban_tui::components::selection_dialog::CarryOverSprintDialog;
    use kanban_tui::components::SelectionDialog;

    let (mut app, _board) = app_with_board_and_sprint_state(LoadState::Loaded(vec![]));
    let dialog = CarryOverSprintDialog { card_count: 0 };
    assert_eq!(dialog.options_count(&app), 0);

    app.mode = AppMode::Dialog(DialogMode::CarryOverSprint);
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::ui::render(&mut app, frame);
    });
    assert!(!output.contains("Sprints not loaded yet"));
    assert!(output.contains("Select target sprint:"));
}

#[test]
fn test_sprint_assign_dialog_distinguishes_not_loaded_from_empty() {
    use kanban_tui::components::selection_dialog::SprintAssignDialog;
    use kanban_tui::components::SelectionDialog;

    let (mut app, _board) = app_with_board_and_sprint_state(LoadState::NotLoaded);
    let dialog = SprintAssignDialog;
    assert_eq!(dialog.options_count(&app), 1);

    app.mode = AppMode::Dialog(DialogMode::AssignCardToSprint);
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::ui::render(&mut app, frame);
    });
    assert!(output.contains("Sprint"));
    assert!(output.contains("not loaded yet"));
}

#[test]
fn test_board_detail_sprints_section_distinguishes_not_loaded_from_empty() {
    let (mut app, _board) = app_with_board_and_sprint_state(LoadState::NotLoaded);
    app.mode = AppMode::BoardDetail;
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::ui::render(&mut app, frame);
    });
    assert!(!output.contains("No sprints yet. Press 'n' to create one!"));
    assert!(output.contains("Sprints not loaded yet"));
}

#[test]
fn test_multi_panel_column_name_lookup_distinguishes_not_loaded_from_unknown() {
    let app = app_with_asymmetric_column_tier(LoadState::NotLoaded);
    let output = helpers::render_widget_to_string(80, 20, |frame| {
        use kanban_tui::render_strategy::{MultiPanelRenderer, RenderStrategy};
        MultiPanelRenderer.render(&app, frame, frame.area());
    });
    assert!(
        !output.contains("Unknown"),
        "an asymmetric NotLoaded board-scoped tier must not fall back to Unknown"
    );
    assert!(output.contains("Not loaded"));
}

#[test]
fn test_multi_panel_column_name_lookup_renders_missing_distinctly() {
    let app = app_with_asymmetric_column_tier(LoadState::Missing);
    let output = helpers::render_widget_to_string(80, 20, |frame| {
        use kanban_tui::render_strategy::{MultiPanelRenderer, RenderStrategy};
        MultiPanelRenderer.render(&app, frame, frame.area());
    });
    assert!(!output.contains("Unknown"));
    assert!(!output.contains("Not loaded"));
    assert!(output.contains("Not found"));
}

#[test]
fn test_multi_panel_column_name_lookup_renders_failed_distinctly() {
    let app = app_with_asymmetric_column_tier(boom());
    let output = helpers::render_widget_to_string(80, 20, |frame| {
        use kanban_tui::render_strategy::{MultiPanelRenderer, RenderStrategy};
        MultiPanelRenderer.render(&app, frame, frame.area());
    });
    assert!(!output.contains("Unknown"));
    assert!(output.contains("Failed"));
}

#[test]
fn test_a_not_loaded_graph_tier_does_not_render_no_parents_or_no_children() {
    let (mut app, _board, _card) = app_with_card_and_graph_state(LoadState::NotLoaded);
    app.mode = AppMode::CardDetail;
    let output = helpers::render_widget_to_string(120, 30, |frame| {
        kanban_tui::ui::render(&mut app, frame);
    });
    assert!(!output.contains("No parents"));
    assert!(!output.contains("No children"));
    assert!(output.contains("not loaded yet"));
    assert!(output.contains("Relationships"));
}

#[test]
fn test_a_failed_graph_tier_renders_the_error_inline_in_the_relationship_panel() {
    let (mut app, _board, _card) = app_with_card_and_graph_state(LoadState::Failed(Arc::new(
        KanbanError::unsupported("boom"),
    )));
    app.mode = AppMode::CardDetail;
    let output = helpers::render_widget_to_string(120, 30, |frame| {
        kanban_tui::ui::render(&mut app, frame);
    });
    assert!(output.contains("failed to load"));
    assert!(output.contains("boom"));
    assert!(!output.contains("No parents"));
}

#[test]
fn test_a_loaded_but_genuinely_empty_graph_still_renders_no_parents_and_no_children() {
    let (mut app, _board, _card) =
        app_with_card_and_graph_state(LoadState::Loaded(DependencyGraph::default()));
    app.mode = AppMode::CardDetail;
    let output = helpers::render_widget_to_string(120, 30, |frame| {
        kanban_tui::ui::render(&mut app, frame);
    });
    assert!(output.contains("No parents"));
    assert!(output.contains("No children"));
}

fn app_with_boards_state(state: LoadState<Vec<Board>>) -> App {
    let mut app = App::test_default();
    let changed = app.model.apply_resolved(Resolved {
        boards: Collection {
            all: state,
            ..Default::default()
        },
        ..Default::default()
    });
    app.controller.resync(&app.model, changed);
    app
}

#[test]
fn test_the_projects_panel_renders_the_not_loaded_marker_instead_of_the_empty_hint() {
    let mut app = App::test_default();
    let output = helpers::render_widget_to_string(400, 30, |frame| {
        kanban_tui::ui::render(&mut app, frame);
    });
    assert!(
        !output.contains("No projects yet"),
        "a NotLoaded boards tier must not render the empty hint, got:\n{output}"
    );
    assert!(
        output.contains("Projects not loaded yet"),
        "a NotLoaded boards tier must render the load-state marker, got:\n{output}"
    );
}

#[test]
fn test_the_projects_panel_renders_the_failed_marker_instead_of_the_empty_hint() {
    let mut app = app_with_boards_state(LoadState::Failed(Arc::new(KanbanError::unsupported(
        "boom",
    ))));
    let output = helpers::render_widget_to_string(400, 30, |frame| {
        kanban_tui::ui::render(&mut app, frame);
    });
    assert!(
        !output.contains("No projects yet"),
        "a Failed boards tier must not render the empty hint, got:\n{output}"
    );
    assert!(
        output.contains("Projects failed to load"),
        "a Failed boards tier must render the load-state marker, got:\n{output}"
    );
    assert!(
        output.contains("boom"),
        "the marker must include the error detail, got:\n{output}"
    );
}

#[test]
fn test_the_projects_panel_still_renders_the_empty_hint_on_a_loaded_empty_boards_tier() {
    let mut app = app_with_boards_state(LoadState::Loaded(Vec::new()));
    let output = helpers::render_widget_to_string(400, 30, |frame| {
        kanban_tui::ui::render(&mut app, frame);
    });
    assert!(
        output.contains("No projects yet. Press 'n' to create one!"),
        "a Loaded-but-empty boards tier is a genuine empty state, got:\n{output}"
    );
}
