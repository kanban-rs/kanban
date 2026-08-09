use crate::app::App;
use crate::components::centered_rect;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_manage_parents_popup(app: &App, frame: &mut Frame) {
    render_relationship_popup(app, frame, "Set Parents");
}

pub fn render_manage_children_popup(app: &App, frame: &mut Frame) {
    render_relationship_popup(app, frame, "Set Children");
}

fn render_relationship_popup(app: &App, frame: &mut Frame, title: &str) {
    let area = centered_rect(60, 70, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(inner);

    render_relationship_search_box(app, frame, chunks[0]);
    render_relationship_card_list(app, frame, chunks[1]);
    render_relationship_instructions(app, frame, chunks[2]);
}

fn render_relationship_search_box(app: &App, frame: &mut Frame, area: ratatui::layout::Rect) {
    let search_border_style = if app.relationship.search_active {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };
    let search_block = Block::default()
        .title("Search")
        .borders(Borders::ALL)
        .border_style(search_border_style);

    let search_text: Line = if app.relationship.search_active {
        Line::from(vec![
            Span::styled(&app.relationship.search, Style::default().fg(Color::White)),
            Span::styled("_", Style::default().fg(Color::Yellow)),
        ])
    } else if app.relationship.search.is_empty() {
        Line::from(Span::styled(
            "/ to search",
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        Line::from(Span::styled(
            &app.relationship.search,
            Style::default().fg(Color::White),
        ))
    };

    let search = Paragraph::new(search_text).block(search_block);
    frame.render_widget(search, area);
}

fn render_relationship_card_list(app: &App, frame: &mut Frame, area: ratatui::layout::Rect) {
    let filtered_cards = app.relationship_filtered_cards();

    let mut lines = vec![];
    for (idx, card_id) in filtered_cards.iter().enumerate() {
        if let Some(card) = app.model.card_by_id(*card_id) {
            let is_selected = app.relationship.selection.get() == Some(idx);
            let is_checked = app.relationship.selected.contains(card_id);

            let checkbox = if is_checked { "[✓]" } else { "[ ]" };

            let style = if is_selected {
                Style::default().fg(Color::White).bg(Color::Blue)
            } else if is_checked {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };

            lines.push(Line::from(Span::styled(
                format!("{} {}", checkbox, card.title),
                style,
            )));
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No eligible cards found",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let list = Paragraph::new(lines);
    frame.render_widget(list, area);
}

fn render_relationship_instructions(app: &App, frame: &mut Frame, area: ratatui::layout::Rect) {
    let instructions_text = if app.relationship.search_active {
        "Type to search | Enter/Esc: exit search"
    } else {
        "j/k: navigate | Space: toggle | /: search | Esc: close"
    };
    let instructions =
        Paragraph::new(instructions_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(instructions, area);
}
