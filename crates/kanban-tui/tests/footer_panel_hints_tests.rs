mod helpers;

use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::AppMode;
use kanban_tui::App;

fn render_footer_text(app: &App) -> String {
    helpers::render_widget_to_string(600, 3, |frame| {
        kanban_tui::components::render_footer(app, frame, frame.area());
    })
}

/// Issue #361: the footer showed `1: panel 1`, `2: panel 2`, ... on every
/// screen. Panel numbers are already rendered as `[1]`/`[2]` indicators in
/// the panel titles, so the footer hints are pure clutter. They must be
/// filtered out of the footer bar.
#[test]
fn test_footer_omits_panel_switch_hints_on_cards_panel() {
    let mut app = App::test_default();
    app.mode = AppMode::Normal;
    app.focus.active = Focus::Cards;

    let footer = render_footer_text(&app);

    assert!(
        (1..=5).all(|n| !footer.contains(&format!("panel {n}"))),
        "footer must not advertise `panel N` hints, got: {footer}"
    );
}

/// The declutter must not swallow genuinely useful hints: on the cards panel
/// the priority shortcut (issue #360) must remain visible in the footer.
#[test]
fn test_footer_keeps_useful_hints_on_cards_panel() {
    let mut app = App::test_default();
    app.mode = AppMode::Normal;
    app.focus.active = Focus::Cards;

    let footer = render_footer_text(&app);

    assert!(
        footer.contains("p: priority"),
        "footer must still show the priority hint, got: {footer}"
    );
}

/// The edit-task (card detail) screen is the worst offender — it defined five
/// `panel N` hints that crowded out useful shortcuts. Those must be gone.
#[test]
fn test_footer_omits_panel_switch_hints_on_card_detail() {
    let mut app = App::test_default();
    app.mode = AppMode::CardDetail;

    let footer = render_footer_text(&app);

    assert!(
        (1..=5).all(|n| !footer.contains(&format!("panel {n}"))),
        "card detail footer must not advertise `panel N` hints, got: {footer}"
    );
}
