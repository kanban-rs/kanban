//! Render for the Manage Labels dialog (`DialogMode::ManageLabels`).
//!
//! Follows the sprint-picker / relationship-list convention: one bordered block
//! containing a single flat, scrollable list where section headers are
//! non-selectable rows (yellow + bold) interspersed between selectable item
//! rows, a blank line separates each section, the cursor is a row-background
//! highlight, and a one-line key-hint footer sits at the bottom.
//!
//! The box hugs its content: it is sized to the number of rows (so a short
//! dialog stays a compact, well-proportioned popup rather than a tall empty
//! box) and capped at 78% of the terminal height so a large registry still
//! scrolls and a small terminal is never overflowed.
use crate::app::{App, LabelRow};
use crate::components::centered_rect_abs;
use crate::components::radio_list::scroll_offset_to_show;
use crate::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Box rows accounted for beyond the list itself: two border rows, a row of
/// top and bottom padding (margin), and the one-line key-hint footer.
const BOX_OVERHEAD: usize = 5;

pub fn render_manage_labels_popup(app: &App, frame: &mut Frame) {
    let frame_area = frame.area();

    let entries = app.label_manager.entries();
    // Blank spacer before every section header except the first.
    let spacers = entries
        .iter()
        .filter(|row| matches!(row, LabelRow::Header(_)))
        .count()
        .saturating_sub(1);
    let visual = entries.len() + spacers;
    // Cap the box at ~78% of the terminal height (long registries scroll)
    // and never exceed the terminal itself / go below a usable minimum.
    let cap = (frame_area.height * 78 / 100).max(10);
    let height = ((visual + BOX_OVERHEAD) as u16)
        .min(cap)
        .min(frame_area.height)
        .max(8);

    let area = centered_rect_abs(72, height, frame_area);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title("Manage Labels")
        .borders(Borders::ALL)
        .border_style(focused_border())
        .style(popup_bg());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(2), Constraint::Length(1)])
        .split(inner);

    render_entries(app, frame, chunks[0]);
    render_instructions(frame, chunks[1]);
}

/// Map a state (navigation) row index to its visual row index, accounting for
/// the spacer lines inserted before each section header except the first.
fn visual_index(state_index: usize, entries: &[LabelRow]) -> usize {
    let spacers_before = entries
        .iter()
        .take(state_index)
        .filter(|row| matches!(row, LabelRow::Header(_)))
        .count()
        .saturating_sub(1);
    state_index + spacers_before
}

/// Build the flat, display-ready list: one line per state entry, with a blank
/// spacer line before every section header except the first.
fn build_visual_lines(
    selected: Option<usize>,
    entries: &[LabelRow],
    app: &App,
    highlight_bg: Color,
) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::with_capacity(entries.len());
    let mut seen_header = false;
    for (i, row) in entries.iter().enumerate() {
        if matches!(row, LabelRow::Header(_)) {
            if seen_header {
                out.push(Line::default());
            }
            seen_header = true;
        }
        out.push(render_row(*row, Some(i) == selected, highlight_bg, app));
    }
    out
}

fn render_entries(app: &App, frame: &mut Frame, area: Rect) {
    if area.height == 0 {
        return;
    }
    let entries = app.label_manager.entries();
    let selected = app.label_manager.selection.get();
    let highlight_bg = app.highlight_bg();

    let lines = build_visual_lines(selected, &entries, app, highlight_bg);
    let total = lines.len();
    let height = area.height as usize;
    let sel_visual = selected
        .map(|s| visual_index(s.min(entries.len().saturating_sub(1)), &entries))
        .unwrap_or(0);
    let scroll = if total == 0 {
        0
    } else {
        scroll_offset_to_show(sel_visual, total, height)
    };
    let paragraph = Paragraph::new(lines).scroll((scroll as u16, 0));
    frame.render_widget(paragraph, area);
}

fn row_style(is_selected: bool, highlight_bg: ratatui::style::Color) -> Style {
    if is_selected {
        normal_text().bg(highlight_bg)
    } else {
        normal_text()
    }
}

fn render_row(
    row: LabelRow,
    is_selected: bool,
    highlight_bg: ratatui::style::Color,
    app: &App,
) -> Line<'static> {
    use std::collections::HashSet;
    match row {
        LabelRow::Header(text) => Line::from(Span::styled(text.to_string(), bold_highlight())),
        LabelRow::Note(text) => Line::from(Span::styled(format!("  {text}"), label_text())),
        LabelRow::Applied(i) => {
            let style = row_style(is_selected, highlight_bg);
            Line::from(vec![
                Span::styled("[✓] ".to_string(), style),
                Span::styled(app.label_manager.applied[i].clone(), style),
            ])
        }
        LabelRow::Registry(i) => {
            let label = &app.label_manager.registry[i];
            let applied: HashSet<&String> = app.label_manager.applied.iter().collect();
            let check = if applied.contains(&label.name) {
                "[✓]"
            } else {
                "[ ]"
            };
            let style = row_style(is_selected, highlight_bg);
            let mut spans: Vec<Span> = vec![
                Span::styled(format!("{check} "), style),
                Span::styled(label.name.clone(), style),
            ];
            if let Some(desc) = &label.description {
                spans.push(Span::styled(format!("  {desc}"), label_text()));
            }
            Line::from(spans)
        }
        LabelRow::Create => {
            let style = row_style(is_selected, highlight_bg);
            Line::from(Span::styled("New label…".to_string(), style))
        }
    }
}

fn render_instructions(frame: &mut Frame, area: Rect) {
    let instructions = Paragraph::new(
        "j/k: navigate   Enter: apply / remove   x: delete   e: edit   n: new   Esc: close",
    )
    .style(
        Style::default()
            .fg(LABEL_TEXT)
            .add_modifier(Modifier::ITALIC),
    );
    frame.render_widget(instructions, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::Label;

    fn state(applied: &[&str], registry: &[&str]) -> App {
        let mut app = App::default();
        app.label_manager.applied = applied.iter().map(|s| s.to_string()).collect();
        app.label_manager.registry = registry.iter().map(|s| Label::named(*s)).collect();
        app
    }

    #[test]
    fn spacers_are_inserted_before_each_section_header_except_the_first() {
        // applied has items, registry has items -> rows:
        // H, A, H, R, R, H, Create  (3 headers -> 2 spacers -> 9 visual lines)
        let app = state(&["bug"], &["bug", "design"]);
        let lines = build_visual_lines(None, &app.label_manager.entries(), &app, Color::DarkGray);
        assert_eq!(lines.len(), 9);
        // The blank spacers sit immediately before the "Labels" and "Create"
        // section headers, i.e. at visual indices 2 and 6.
        let blanks: Vec<usize> = (0..lines.len())
            .filter(|i| lines[*i].spans.iter().all(|s| s.content.as_ref() == ""))
            .collect();
        assert_eq!(blanks, vec![2, 6]);
    }

    #[test]
    fn empty_registry_and_applied_still_separate_three_sections() {
        // H, note, H, note, H, Create (3 headers -> 2 spacers -> 8 visual lines)
        let app = state(&[], &[]);
        let lines = build_visual_lines(None, &app.label_manager.entries(), &app, Color::DarkGray);
        assert_eq!(lines.len(), 8);
    }
}
