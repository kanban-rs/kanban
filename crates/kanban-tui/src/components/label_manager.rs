//! Render for the Manage Labels dialog (`DialogMode::ManageLabels`).
//!
//! Two labelled sections plus a create row and an instruction line:
//! - Applied labels (the card's `Card.tags`), upper;
//! - All registered labels (the global registry), lower;
//! - a "new label" input row that opens the external editor to create one.
//!
//! The single shared cursor is `app.label_manager.selection`; whichever row it
//! points at is highlighted, so navigation reads as one continuous list.
use crate::app::App;
use crate::components::centered_rect;
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

    let title = Block::default()
        .title("Manage Labels")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));
    let inner = title.inner(area);
    frame.render_widget(title, area);

    let applied_len = app.label_manager.applied.len() as u16;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            // Applied block (content + its borders).
            Constraint::Length(applied_len.saturating_add(3).max(3)),
            Constraint::Length(1),
            // Registry block takes the remaining vertical space.
            Constraint::Min(3),
            // Create input row (a bordered box, 3 lines tall).
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(inner);

    render_applied_section(app, frame, chunks[0]);
    render_registry_section(app, frame, chunks[2]);
    render_create_row(app, frame, chunks[3]);
    render_instructions(frame, chunks[4]);
}

fn render_applied_section(app: &App, frame: &mut Frame, area: Rect) {
    let selected = app.label_manager.selection.get();
    let mut lines: Vec<Line> = Vec::with_capacity(app.label_manager.applied.len().max(1));
    for (i, name) in app.label_manager.applied.iter().enumerate() {
        let style = if selected == Some(i) {
            Style::default().fg(Color::White).bg(app.highlight_bg())
        } else {
            Style::default().fg(Color::Green)
        };
        lines.push(Line::from(Span::styled(
            format!("    \u{2713} {name}"),
            style,
        )));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "    (none on this card)",
            Style::default().fg(Color::DarkGray),
        )));
    }
    let block = Block::default()
        .title("APPLIED TO THIS CARD   (Enter/x: remove, e: edit)")
        .borders(Borders::ALL);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_registry_section(app: &App, frame: &mut Frame, area: Rect) {
    use std::collections::HashSet;
    let applied: HashSet<&String> = app.label_manager.applied.iter().collect();
    let base = app.label_manager.applied.len();
    let selected = app.label_manager.selection.get();
    let mut lines: Vec<Line> = Vec::with_capacity(app.label_manager.registry.len().max(1));
    for (i, label) in app.label_manager.registry.iter().enumerate() {
        let style = if selected == Some(base + i) {
            Style::default().fg(Color::White).bg(app.highlight_bg())
        } else {
            Style::default().fg(Color::White)
        };
        let marker = if applied.contains(&label.name) {
            "\u{2713}"
        } else {
            " "
        };
        let color_dot = label
            .color
            .as_deref()
            .map(|c| format!("  {c}"))
            .unwrap_or_default();
        let desc = label
            .description
            .as_deref()
            .map(|d| format!("  {d}"))
            .unwrap_or_default();
        lines.push(Line::from(vec![
            Span::styled(format!("    {marker} {}", label.name), style),
            Span::styled(color_dot, Style::default().fg(Color::Cyan)),
            Span::styled(desc, Style::default().fg(Color::DarkGray)),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "    (no labels defined yet)",
            Style::default().fg(Color::DarkGray),
        )));
    }
    let block = Block::default()
        .title("ALL LABELS   (Enter/space: set on card, x: delete, e: edit)")
        .borders(Borders::ALL);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_create_row(app: &App, frame: &mut Frame, area: Rect) {
    let create_idx = app.label_manager.total_rows().saturating_sub(1);
    let is_sel = app.label_manager.selection.get() == Some(create_idx);
    let border_style = if is_sel {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };
    let block = Block::default()
        .title("NEW LABEL   (Enter/n/e: create)")
        .borders(Borders::ALL)
        .border_style(border_style);
    let text = Line::from(vec![
        Span::styled(
            "    [ + ] ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "open editor to create (name, color, description)",
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    frame.render_widget(Paragraph::new(text).block(block), area);
}

fn render_instructions(frame: &mut Frame, area: Rect) {
    let text =
        "j/k: navigate  |  Enter: apply/remove  |  x: delete  |  e: edit  |  n: new  |  Esc: close";
    let instructions = Paragraph::new(text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(instructions, area);
}
