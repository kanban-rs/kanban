use kanban_view::scroll_indicators::{scroll_indicator, ScrollDirection, ScrollIndicator};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

/// Format a `kanban-view` scroll indicator as the TUI's indicator line text,
/// e.g. `"  2 Tasks above"`. The two leading spaces align the message with the
/// indented list items above and below it.
pub fn indicator_text(indicator: &ScrollIndicator, label: &str) -> String {
    let plural = if indicator.is_plural() { "s" } else { "" };
    let direction = match indicator.direction {
        ScrollDirection::Above => "above",
        ScrollDirection::Below => "below",
    };
    format!("  {} {}{} {}", indicator.count, label, plural, direction)
}

fn render_indicator<'a>(
    show: bool,
    count: usize,
    direction: ScrollDirection,
    label: &str,
) -> Option<Line<'a>> {
    scroll_indicator(show, count, direction).map(|indicator| {
        Line::from(Span::styled(
            indicator_text(&indicator, label),
            Style::default().fg(Color::DarkGray),
        ))
    })
}

/// Render an "N items above" indicator line, or `None` if not shown.
pub fn render_above_indicator<'a>(show: bool, count: usize, label: &str) -> Option<Line<'a>> {
    render_indicator(show, count, ScrollDirection::Above, label)
}

/// Render an "N items below" indicator line, or `None` if not shown.
pub fn render_below_indicator<'a>(show: bool, count: usize, label: &str) -> Option<Line<'a>> {
    render_indicator(show, count, ScrollDirection::Below, label)
}

/// Render scroll indicators for any scrollable list.
///
/// Produces 0, 1, or 2 lines indicating how many items lie above or below
/// the current viewport. `label` is the singular noun used in the message
/// (e.g. `"Task"`, `"item"`, `"entry"`).
pub fn render_scroll_indicators<'a>(
    show_above: bool,
    items_above: usize,
    show_below: bool,
    items_below: usize,
    label: &str,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    lines.extend(render_above_indicator(show_above, items_above, label));
    lines.extend(render_below_indicator(show_below, items_below, label));
    lines
}
