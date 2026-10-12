use crate::components::{centered_rect, render_popup_with_block, styled_list_item, ListItemConfig};
use crate::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::Color,
    widgets::{List, ListItem, Paragraph},
    Frame,
};

pub fn render_selection_popup_with_list_items(
    frame: &mut Frame,
    title: &str,
    items: Vec<ListItem>,
    width: u16,
    height: u16,
) {
    let area = centered_rect(width, height, frame.area());
    let list = List::new(items).block(
        ratatui::widgets::Block::default()
            .title(title)
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(focused_border()),
    );

    frame.render_widget(ratatui::widgets::Clear, area);
    frame.render_widget(list, area);
}

#[allow(clippy::too_many_arguments)]
pub fn render_selection_popup_with_lines<'a, I, F>(
    frame: &mut Frame,
    title: &str,
    label: Option<&str>,
    items: I,
    format_fn: F,
    selected_idx: Option<usize>,
    active_idx: Option<usize>,
    width: u16,
    height: u16,
    highlight_bg: Color,
) where
    I: IntoIterator,
    I::Item: 'a,
    F: Fn(usize, &I::Item, bool, bool) -> (String, Option<String>),
{
    let inner = render_popup_with_block(frame, title, width, height);

    let chunks = if label.is_some() {
        Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([Constraint::Min(0)])
            .split(inner)
    };

    if let Some(label_text) = label {
        let label_widget = Paragraph::new(label_text).style(highlight_text());
        frame.render_widget(label_widget, chunks[0]);
    }

    let list_chunk = if label.is_some() {
        chunks[1]
    } else {
        chunks[0]
    };

    let mut lines = vec![];
    for (idx, item) in items.into_iter().enumerate() {
        let is_selected = selected_idx == Some(idx);
        let is_active = active_idx == Some(idx);
        let (text, suffix) = format_fn(idx, &item, is_selected, is_active);

        let config = ListItemConfig::new(highlight_bg)
            .selected(is_selected)
            .focused(true)
            .active(is_active);

        let mut line_text = text;
        if let Some(suffix_text) = suffix {
            line_text.push_str(&suffix_text);
        }

        lines.push(styled_list_item(line_text, &config));
    }

    let list = Paragraph::new(lines);
    frame.render_widget(list, list_chunk);
}
