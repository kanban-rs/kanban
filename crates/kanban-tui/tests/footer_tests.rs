mod helpers;

use kanban_tui::app::focus::Focus;
use kanban_tui::App;

fn render_footer_to_string(app: &App) -> String {
    use ratatui::layout::Rect;
    helpers::render_widget_to_string(120, 3, |frame| {
        let area = Rect::new(0, 0, 120, 3);
        kanban_tui::components::render_footer(app, frame, area);
    })
}

#[test]
fn test_render_footer_normal_mode_renders_without_panic() {
    let app = App::test_default();
    let output = render_footer_to_string(&app);
    // Normal mode renders keybindings in "key: description" format
    assert!(
        output.contains(": "),
        "Footer should contain at least one keybinding hint"
    );
}

#[test]
fn test_render_footer_search_active_shows_search_query() {
    let mut app = App::test_default();
    app.focus.active = Focus::Boards;
    app.filter.search.is_active = true;
    for c in "hello".chars() {
        app.filter.search.input.insert_char(c);
    }
    let output = render_footer_to_string(&app);
    assert!(output.contains("/hello"), "Footer should show search query");
}

#[test]
fn test_footer_shows_board_query_after_commit_when_board_search_active() {
    let mut app = App::test_default();
    app.focus.active = Focus::Boards;
    app.filter.board_search.is_active = true;
    for c in "alpha".chars() {
        app.filter.board_search.input.insert_char(c);
    }
    let output = render_footer_to_string(&app);
    assert!(
        output.contains("/alpha"),
        "Footer should echo the committed board search query, not render blank"
    );
}

#[test]
fn test_footer_shows_board_query_while_typing_when_board_search_active() {
    use kanban_tui::app::mode::AppMode;
    let mut app = App::test_default();
    app.focus.active = Focus::Boards;
    app.filter.board_search.activate();
    for c in "alpha".chars() {
        app.filter.board_search.input.insert_char(c);
    }
    app.push_mode(AppMode::Search);
    let output = render_footer_to_string(&app);
    assert!(
        output.contains("/alpha"),
        "Footer should echo the board search query while typing, not the empty card search field"
    );
}

#[test]
fn test_render_footer_search_mode_renders_without_panic() {
    use kanban_tui::app::mode::AppMode;
    let mut app = App::test_default();
    app.focus.active = Focus::Boards;
    app.push_mode(AppMode::Search);
    let output = render_footer_to_string(&app);
    // Search mode renders documented keybindings "ESC: clear | Enter: apply"
    assert!(
        output.contains("ESC"),
        "Footer should show ESC keybinding in search mode"
    );
    assert!(
        output.contains("Enter"),
        "Footer should show Enter keybinding in search mode"
    );
}

#[test]
fn test_render_footer_sprint_detail_mode_includes_component_help() {
    use kanban_tui::app::AppMode;
    let mut app = App::test_default();
    app.push_mode(AppMode::SprintDetail);
    let output = render_footer_to_string(&app);
    assert!(!output.trim().is_empty());
    // Sprint detail mode appends component help text after the keybinding section
    // The separator "|" appears at least twice (between keybindings and after component help)
    assert!(output.matches('|').count() >= 2);
}

#[test]
fn test_render_footer_multiselect_active_shows_select_prefix() {
    use uuid::Uuid;
    let mut app = App::test_default();
    app.multi_select.selection_mode_active = true;
    app.multi_select.selected_cards.insert(Uuid::new_v4());
    let output = render_footer_to_string(&app);
    assert!(output.contains("SELECT"));
    assert!(output.contains('1') || output.contains("(1"));
}
