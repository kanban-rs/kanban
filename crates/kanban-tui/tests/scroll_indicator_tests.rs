use kanban_tui::scroll_indicators::{
    indicator_text, render_above_indicator, render_below_indicator,
};
use kanban_view::scroll_indicators::ScrollIndicator;

#[test]
fn test_indicator_text_above_pluralizes_correctly() {
    assert_eq!(
        indicator_text(&ScrollIndicator::above(1), "item"),
        "  1 item above"
    );
    assert_eq!(
        indicator_text(&ScrollIndicator::above(2), "item"),
        "  2 items above"
    );
}

#[test]
fn test_indicator_text_below_pluralizes_correctly() {
    assert_eq!(
        indicator_text(&ScrollIndicator::below(1), "item"),
        "  1 item below"
    );
    assert_eq!(
        indicator_text(&ScrollIndicator::below(2), "item"),
        "  2 items below"
    );
}

#[test]
fn test_indicator_text_keeps_the_two_space_terminal_indent() {
    assert!(indicator_text(&ScrollIndicator::above(3), "Task").starts_with("  "));
    assert_eq!(
        indicator_text(&ScrollIndicator::above(3), "Task"),
        "  3 Tasks above"
    );
}

#[test]
fn test_render_above_indicator_hidden_returns_none() {
    assert!(render_above_indicator(false, 4, "Task").is_none());
}

#[test]
fn test_render_above_indicator_renders_unchanged_terminal_text() {
    let line = render_above_indicator(true, 4, "Task").expect("indicator should render");
    assert_eq!(line.spans[0].content, "  4 Tasks above");
}

#[test]
fn test_render_below_indicator_renders_unchanged_terminal_text() {
    let line = render_below_indicator(true, 1, "Task").expect("indicator should render");
    assert_eq!(line.spans[0].content, "  1 Task below");
}
