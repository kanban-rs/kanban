/// Render an "N items above" indicator line, or `None` if not shown.
pub fn render_above_indicator<'a>(
    show: bool,
    count: usize,
    label: &str,
) -> Option<ratatui::text::Line<'a>> {
    use kanban_view::scroll_indicators::above_indicator_text;
    use ratatui::style::{Color, Style};
    use ratatui::text::{Line, Span};

    if show {
        Some(Line::from(Span::styled(
            above_indicator_text(count, label),
            Style::default().fg(Color::DarkGray),
        )))
    } else {
        None
    }
}

/// Render an "N items below" indicator line, or `None` if not shown.
pub fn render_below_indicator<'a>(
    show: bool,
    count: usize,
    label: &str,
) -> Option<ratatui::text::Line<'a>> {
    use kanban_view::scroll_indicators::below_indicator_text;
    use ratatui::style::{Color, Style};
    use ratatui::text::{Line, Span};

    if show {
        Some(Line::from(Span::styled(
            below_indicator_text(count, label),
            Style::default().fg(Color::DarkGray),
        )))
    } else {
        None
    }
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
) -> Vec<ratatui::text::Line<'a>> {
    let mut lines = Vec::new();
    lines.extend(render_above_indicator(show_above, items_above, label));
    lines.extend(render_below_indicator(show_below, items_below, label));
    lines
}
