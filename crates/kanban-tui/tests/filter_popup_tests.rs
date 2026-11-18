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
fn test_render_filter_options_popup_shows_labels_section() {
    let app = setup_app_with_filter_dialog();
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::components::render_filter_options_popup(&app, frame);
    });
    assert!(output.contains("Labels"));
    assert!(output.contains("(no labels in registry)"));
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

#[test]
fn test_label_filter_excludes_non_matching_cards() {
    use kanban_domain::{CreateCardOptions, KanbanOperations};

    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let card_bug = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Bug card".to_string(),
            CreateCardOptions {
                labels: Some(vec!["bug".to_string()]),
                ..Default::default()
            },
        )
        .unwrap();
    let card_feature = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Feature card".to_string(),
            CreateCardOptions {
                labels: Some(vec!["feature".to_string()]),
                ..Default::default()
            },
        )
        .unwrap();

    app.selection.active_board_id = Some(board.id);
    app.reload_model();

    // Without an active label filter both cards are visible.
    app.prepare_frame();
    let all = app
        .view
        .strategy
        .get_active_task_list()
        .map(|l| l.cards.clone())
        .unwrap_or_default();
    assert!(
        all.contains(&card_bug.id) && all.contains(&card_feature.id),
        "no label filter: both cards must be visible"
    );

    // Setting the label filter to "bug" hides the "feature" card.
    app.filter.active_labels = Some(vec!["bug".to_string()]);
    app.prepare_frame();
    let filtered = app
        .view
        .strategy
        .get_active_task_list()
        .map(|l| l.cards.clone())
        .unwrap_or_default();
    assert_eq!(filtered.len(), 1, "label filter should show exactly 1 card");
    assert!(
        filtered.contains(&card_bug.id),
        "card with the 'bug' label must be visible"
    );
    assert!(
        !filtered.contains(&card_feature.id),
        "card without the 'bug' label must be hidden"
    );

    // Clearing the label filter restores both cards.
    app.filter.active_labels = None;
    app.prepare_frame();
    let restored = app
        .view
        .strategy
        .get_active_task_list()
        .map(|l| l.cards.clone())
        .unwrap_or_default();
    assert!(
        restored.contains(&card_bug.id) && restored.contains(&card_feature.id),
        "clearing the label filter shows all cards again"
    );
}
