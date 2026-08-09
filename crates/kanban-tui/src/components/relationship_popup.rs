use crate::app::App;
use crate::components::centered_rect;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
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

fn relationship_popup_layout(frame_area: Rect) -> std::rc::Rc<[Rect]> {
    let area = centered_rect(60, 70, frame_area);
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);

    Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(inner)
}

/// The card list's viewport height, computed from the current frame area.
/// Mirrors `render_relationship_popup`'s layout math so key handlers can
/// scroll the list into view without needing a live `Frame`.
pub fn relationship_popup_viewport_height(frame_area: Rect) -> usize {
    relationship_popup_layout(frame_area)[1].height as usize
}

fn render_relationship_popup(app: &App, frame: &mut Frame, title: &str) {
    let area = centered_rect(60, 70, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));

    frame.render_widget(block, area);

    let chunks = relationship_popup_layout(frame.area());

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

fn render_relationship_card_list(app: &App, frame: &mut Frame, area: Rect) {
    let filtered_cards = app.relationship_filtered_cards();

    let mut lines = vec![];

    if filtered_cards.is_empty() {
        lines.push(Line::from(Span::styled(
            "No eligible cards found",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        // Built from `filtered_cards.len()` rather than
        // `picker_list.get_render_info` directly: the picker's item count
        // changes on every search keystroke, and rendering must reflect the
        // current filtered set even if a caller mutated `search`/`card_ids`
        // without also calling `picker_list.update_item_count`.
        let mut page = kanban_core::Page::new(filtered_cards.len());
        page.set_scroll_offset(app.relationship.picker_list.get_scroll_offset());
        let adjusted_height = page.get_adjusted_viewport_height(area.height as usize);
        let page_info = page.get_page_info(adjusted_height);

        lines.extend(crate::scroll_indicators::render_above_indicator(
            page_info.show_above_indicator,
            page_info.items_above,
            "card",
        ));

        for &idx in &page_info.visible_indices {
            if let Some(card_id) = filtered_cards.get(idx) {
                if let Some(card) = app.model.card_by_id(*card_id) {
                    let is_selected = app.relationship.picker_list.selection.get() == Some(idx);
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
        }

        lines.extend(crate::scroll_indicators::render_below_indicator(
            page_info.show_below_indicator,
            page_info.items_below,
            "card",
        ));
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
