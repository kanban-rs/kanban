use crate::app::App;
use crate::components::centered_rect;
use crate::theme::*;
use kanban_core::pagination::scroll_offset_to_keep_visible;
use kanban_view::filters::FilterDialogState;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_filter_options_popup(app: &App, frame: &mut Frame) {
    let area = centered_rect(70, 75, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title("Filter Options")
        .borders(Borders::ALL)
        .border_style(focused_border());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(inner);

    if let Some(ref dialog_state) = app.filter.dialog_state {
        render_filter_sprints_section(app, frame, chunks[0], dialog_state);
        render_filter_date_range_section(frame, chunks[1], dialog_state.section_index);
        render_filter_tags_section(frame, chunks[2], dialog_state.section_index);
    }
}

fn render_filter_sprints_section(
    app: &App,
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    dialog_state: &FilterDialogState,
) {
    let section_index = dialog_state.section_index;

    let mut sprint_lines = vec![Line::from(Span::styled(
        "Sprints",
        if section_index == 0 {
            bold_highlight()
        } else {
            normal_text()
        },
    ))];

    let unassigned_cursor = if section_index == 0 && dialog_state.item_selection == 0 {
        "> "
    } else {
        "  "
    };

    sprint_lines.push(Line::from(vec![
        Span::raw(unassigned_cursor),
        Span::styled(
            if dialog_state.filters.show_unassigned_sprints {
                "[✓]"
            } else {
                "[ ]"
            },
            normal_text(),
        ),
        Span::raw(" "),
        Span::styled("Show cards with unassigned sprints", normal_text()),
    ]));

    sprint_lines.push(Line::from(Span::styled(
        "─────────────────────────",
        label_text(),
    )));

    if let Some(board) = app.active_board() {
        {
            let sprints = app.model.sprints();
            let board_sprints: Vec<_> = sprints.iter().filter(|s| s.board_id == board.id).collect();

            if board_sprints.is_empty() {
                sprint_lines.push(Line::from(Span::styled(
                    "  (no sprints available)",
                    label_text(),
                )));
            } else {
                for (idx, sprint) in board_sprints.iter().enumerate() {
                    let is_selected = dialog_state
                        .filters
                        .selected_sprint_ids
                        .contains(&sprint.id);
                    let cursor = if section_index == 0 && dialog_state.item_selection == idx + 1 {
                        "> "
                    } else {
                        "  "
                    };

                    sprint_lines.push(Line::from(vec![
                        Span::raw(cursor),
                        Span::styled(if is_selected { "[✓]" } else { "[ ]" }, normal_text()),
                        Span::raw(" "),
                        Span::styled(sprint.formatted_name(board, "sprint"), normal_text()),
                    ]));
                }
            }
        }
    }

    let selected_line_idx = if dialog_state.item_selection == 0 {
        1
    } else {
        dialog_state.item_selection + 2
    };
    let viewport_height = area.height.saturating_sub(2) as usize;
    let scroll = scroll_offset_to_keep_visible(
        dialog_state.item_scroll.get(),
        selected_line_idx,
        viewport_height,
    );
    dialog_state.item_scroll.set(scroll);
    let section = Paragraph::new(sprint_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if section_index == 0 {
                    focused_border()
                } else {
                    Style::default()
                }),
        )
        .scroll((scroll as u16, 0));
    frame.render_widget(section, area);
}

fn render_filter_date_range_section(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    section_index: usize,
) {
    let date_lines = vec![
        Line::from(Span::styled(
            "Date Range (Future)",
            if section_index == 1 {
                bold_highlight()
            } else {
                label_text()
            },
        )),
        Line::from(Span::styled(
            "  Filter by last updated or created date",
            label_text(),
        )),
    ];

    let section =
        Paragraph::new(date_lines).block(Block::default().borders(Borders::ALL).border_style(
            if section_index == 1 {
                focused_border()
            } else {
                Style::default()
            },
        ));
    frame.render_widget(section, area);
}

fn render_filter_tags_section(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    section_index: usize,
) {
    let tag_lines = vec![
        Line::from(Span::styled(
            "Tags (Future)",
            if section_index == 2 {
                bold_highlight()
            } else {
                label_text()
            },
        )),
        Line::from(Span::styled("  Filter cards by tags", label_text())),
    ];

    let section =
        Paragraph::new(tag_lines).block(Block::default().borders(Borders::ALL).border_style(
            if section_index == 2 {
                focused_border()
            } else {
                Style::default()
            },
        ));
    frame.render_widget(section, area);
}
