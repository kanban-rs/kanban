//! Render for the Manage Labels dialog (`DialogMode::ManageLabels`).
use crate::app::App;
use crate::components::centered_rect;
use crate::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
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

    let selection = app.label_manager.selection.get();
    let applied_count = app.label_manager.applied.len();
    let create_idx = app.label_manager.total_rows().saturating_sub(1);

    let applied_active = selection.is_some_and(|s| s < applied_count);
    let registry_active = selection.is_some_and(|s| s >= applied_count && s < create_idx);
    let create_active = selection == Some(create_idx);

    let applied_height = (applied_count.max(1) + 3) as u16;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(applied_height),
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(4),
            Constraint::Length(1),
        ])
        .split(inner);

    render_applied_section(app, frame, chunks[0], applied_active);
    render_registry_section(app, frame, chunks[2], registry_active);
    render_create_row(app, frame, chunks[3], create_active);
    render_instructions(frame, chunks[4]);
}

fn section_border(active: bool) -> Style {
    if active {
        focused_border()
    } else {
        Style::default()
    }
}

fn row_style(is_selected: bool, highlight_bg: Color) -> Style {
    if is_selected {
        normal_text().bg(highlight_bg)
    } else {
        normal_text()
    }
}

fn render_applied_section(app: &App, frame: &mut Frame, area: Rect, active: bool) {
    let highlight_bg = app.highlight_bg();
    let selected = app.label_manager.selection.get();

    let mut lines: Vec<Line> = vec![Line::from(Span::styled(
        "Applied to this card",
        if active {
            bold_highlight()
        } else {
            normal_text()
        },
    ))];

    for (i, name) in app.label_manager.applied.iter().enumerate() {
        let is_selected = selected == Some(i);
        let style = row_style(is_selected, highlight_bg);
        let cursor = if is_selected { "> " } else { "  " };
        lines.push(Line::from(vec![
            Span::styled(cursor.to_string(), style),
            Span::styled("[✓] ".to_string(), style),
            Span::styled(name.clone(), style),
        ]));
    }
    if app.label_manager.applied.is_empty() {
        lines.push(Line::from(Span::styled("  (none)", label_text())));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(section_border(active));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_registry_section(app: &App, frame: &mut Frame, area: Rect, active: bool) {
    use std::collections::HashSet;
    let highlight_bg = app.highlight_bg();
    let selected = app.label_manager.selection.get();
    let base = app.label_manager.applied.len();
    let applied: HashSet<&String> = app.label_manager.applied.iter().collect();

    let mut lines: Vec<Line> = vec![Line::from(Span::styled(
        "Labels",
        if active {
            bold_highlight()
        } else {
            normal_text()
        },
    ))];

    for (i, label) in app.label_manager.registry.iter().enumerate() {
        let flat_idx = base + i;
        let is_selected = selected == Some(flat_idx);
        let style = row_style(is_selected, highlight_bg);
        let cursor = if is_selected { "> " } else { "  " };
        let check = if applied.contains(&label.name) {
            "[✓]"
        } else {
            "[ ]"
        };

        let mut spans: Vec<Span> = vec![
            Span::styled(cursor.to_string(), style),
            Span::styled(format!("{check} "), style),
            Span::styled(label.name.clone(), style),
        ];
        if let Some(desc) = &label.description {
            spans.push(Span::styled(format!("  {desc}"), label_text()));
        }
        lines.push(Line::from(spans));
    }
    if app.label_manager.registry.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (none defined yet)",
            label_text(),
        )));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(section_border(active));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_create_row(_app: &App, frame: &mut Frame, area: Rect, active: bool) {
    let border = if active {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let lines = vec![
        Line::from(Span::styled(
            "Create",
            if active {
                bold_highlight()
            } else {
                normal_text()
            },
        )),
        Line::from(Span::styled("  Enter or n: open editor", label_text())),
    ];

    let block = Block::default().borders(Borders::ALL).border_style(border);
    frame.render_widget(Paragraph::new(lines).block(block), area);
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
