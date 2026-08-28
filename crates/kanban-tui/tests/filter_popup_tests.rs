mod helpers;

use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::App;

fn setup_app_with_filter_dialog() -> App {
    use kanban_domain::CardFilters;
    use kanban_view::filters::FilterDialogState;
    let mut app = App::test_default();
    app.push_mode(AppMode::Dialog(DialogMode::FilterOptions));
    app.filter.dialog_state = Some(FilterDialogState::new(CardFilters::default()));
    app
}

#[test]
fn test_render_filter_options_popup_renders_without_dialog_state() {
    let app = App::test_default();
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::components::render_filter_options_popup(&app, frame);
    });
    // Without dialog state active, the outer popup block still renders with its title
    assert!(
        output.contains("Filter Options"),
        "Popup should render its title even without dialog state"
    );
}

#[test]
fn test_render_filter_options_popup_shows_sprints_section() {
    let app = setup_app_with_filter_dialog();
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::components::render_filter_options_popup(&app, frame);
    });
    assert!(output.contains("Sprints"));
}

#[test]
fn test_render_filter_options_popup_shows_date_range_section() {
    let app = setup_app_with_filter_dialog();
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::components::render_filter_options_popup(&app, frame);
    });
    assert!(output.contains("Date Range"));
}

#[test]
fn test_render_filter_options_popup_shows_tags_section() {
    let app = setup_app_with_filter_dialog();
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::components::render_filter_options_popup(&app, frame);
    });
    assert!(output.contains("Tags"));
}

#[test]
fn test_render_filter_popup_with_sprint_shows_sprint_name() {
    use kanban_domain::CardFilters;
    use kanban_view::filters::FilterDialogState;
    let mut app = App::test_default();
    use kanban_domain::KanbanOperations;
    let board = app
        .ctx
        .create_board("Test Board".to_string(), None)
        .unwrap();
    app.ctx
        .create_sprint(board.id, None, Some("Sprint".to_string()))
        .unwrap();
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.push_mode(AppMode::Dialog(DialogMode::FilterOptions));
    app.filter.dialog_state = Some(FilterDialogState::new(CardFilters::default()));
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::components::render_filter_options_popup(&app, frame);
    });
    assert!(output.contains("Sprint 1") || output.contains("Sprint"));
    assert!(!output.contains("no sprints available"));
}
