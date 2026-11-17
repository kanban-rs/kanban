//! Render for the Manage Labels dialog (`DialogMode::ManageLabels`).
//!
//! Follows the sprint-picker / relationship-list convention: one bordered block
//! containing a single flat, scrollable list where section headers are
//! non-selectable rows (yellow + bold) interspersed between selectable item
//! rows, plus a one-line key-hint footer.
use crate::app::{App, LabelRow};
use crate::components::centered_rect;
use crate::components::radio_list::scroll_offset_to_show;
use crate::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_manage_labels_popup(app: &App, frame: &mut Frame) {
    let area = centered_rect(72, 78, frame.area());
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
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(inner);

    render_entries(app, frame, chunks[0]);
    render_instructions(frame, chunks[1]);
}

fn render_entries(app: &App, frame: &mut Frame, area: Rect) {
    if area.height == 0 {
        return;
    }
    let entries = app.label_manager.entries();
    let selected = app.label_manager.selection.get();
    let highlight_bg = app.highlight_bg();

    let lines: Vec<Line> = entries
        .iter()
        .enumerate()
        .map(|(i, row)| render_row(*row, selected == Some(i), highlight_bg, app))
        .collect();

    let total = lines.len();
    let height = area.height as usize;
    let scroll = if total == 0 {
        0
    } else {
        let sel = selected.unwrap_or(0).min(total - 1);
        scroll_offset_to_show(sel, total, height)
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
