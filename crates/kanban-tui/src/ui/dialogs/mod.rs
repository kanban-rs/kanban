mod boards;
mod cards;
mod columns;
mod sprints;

pub(super) use boards::*;
pub(super) use cards::*;
pub(super) use columns::*;
pub(super) use sprints::*;

use crate::components::*;
use crate::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Shared modal confirmation popup used by the delete dialogs: a centered
/// bordered box with a yellow body message and the fixed key hint. Kept in one
/// place so the two (board/column) delete confirmations cannot drift.
pub(crate) fn render_confirm_popup(frame: &mut Frame, title: &str, body: String) {
    let area = centered_rect(60, 40, frame.area());
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(title.to_string())
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);
    frame.render_widget(
        Paragraph::new(body).style(Style::default().fg(Color::Yellow)),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new("Press ENTER/y to delete, n/ESC to cancel").style(label_text()),
        chunks[1],
    );
}
