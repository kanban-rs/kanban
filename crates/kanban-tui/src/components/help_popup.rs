use crate::app::App;
use crate::components::centered_rect;
use crate::components::ListItemConfig;
use crate::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn help_popup_viewport_height(frame_area: Rect) -> usize {
    let popup = centered_rect(80, 80, frame_area);
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .horizontal_margin(2)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(inner);
    chunks[1].height as usize
}

pub fn render_help_popup(app: &App, frame: &mut Frame) {
    use crate::keybindings::KeybindingRegistry;

    let area = centered_rect(80, 80, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title("Help - Keybindings for Current Context")
        .borders(Borders::ALL)
        .border_style(focused_border());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .horizontal_margin(2)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(inner);

    let provider = KeybindingRegistry::get_provider(app);
    let context = provider.get_context();

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                context.name.clone(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ]),
        chunks[0],
    );

    let raw_height = help_popup_viewport_height(frame.area());

    let selected_idx = app.ui_state.help_list.get_selected_index();
    let highlight_bg = app.highlight_bg();

    let adjusted_height = app
        .ui_state
        .help_list
        .get_adjusted_viewport_height(raw_height);
    let page_info = app.ui_state.help_list.get_render_info(adjusted_height);

    let mut rendered_lines: Vec<Line> = crate::scroll_indicators::render_above_indicator(
        page_info.show_above_indicator,
        page_info.items_above,
        "item",
    )
    .into_iter()
    .collect();

    let visible_lines: Vec<Line> = page_info
        .visible_indices
        .iter()
        .filter_map(|&i| {
            let binding = context.bindings.get(i)?;
            let is_selected = selected_idx == Some(i);
            let config = ListItemConfig::new(highlight_bg)
                .selected(is_selected)
                .focused(true);
            let prefix = config.item_prefix();
            let style = config.item_style();
            Some(Line::from(vec![
                Span::styled(prefix.to_string(), style),
                Span::styled(binding.key.to_string(), Style::default().fg(Color::Yellow)),
                Span::raw(" "),
                Span::styled(binding.description.clone(), style),
            ]))
        })
        .collect();
    rendered_lines.extend(visible_lines);
    rendered_lines.extend(crate::scroll_indicators::render_below_indicator(
        page_info.show_below_indicator,
        page_info.items_below,
        "item",
    ));

    frame.render_widget(Paragraph::new(rendered_lines), chunks[1]);

    let footer = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "j/k or ↑↓: navigate | Enter: activate | ESC or ?: close",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )),
    ]);
    frame.render_widget(footer, chunks[2]);
}
