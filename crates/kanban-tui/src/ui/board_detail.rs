use crate::app::{App, BoardFocus};
use crate::components::*;
use crate::theme::*;
use kanban_core::pagination::scroll_offset_to_keep_visible;
use kanban_domain::card_lifecycle::sorted_board_columns;
use kanban_domain::{Sprint, SprintStatus};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub(super) fn render_board_detail_view(app: &mut App, frame: &mut Frame, area: Rect) {
    // Resolve the board by identity so board detail works for a live OR archived
    // board without branching — a board is a board. Cloned so the borrow of
    // `app` doesn't outlive the `&mut App` the columns list needs below.
    if let Some(board) = app.board_in_context().cloned() {
        {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(5),
                    Constraint::Length(8),
                    Constraint::Percentage(30),
                    Constraint::Percentage(35),
                    Constraint::Percentage(35),
                ])
                .split(area);

            render_board_name_field(app, &board, frame, chunks[0]);
            render_board_description_field(app, &board, frame, chunks[1]);
            render_board_settings_section(app, &board, frame, chunks[2]);
            render_board_sprints_list(app, &board, frame, chunks[3]);
            render_board_columns_list(app, &board, frame, chunks[4]);
        }
    }
}

fn render_board_name_field(app: &App, board: &kanban_domain::Board, frame: &mut Frame, area: Rect) {
    let name_config = FieldSectionConfig::new("Project Name")
        .with_focus_indicator("Project Name [1]")
        .focused(app.focus.board_focus == BoardFocus::Name);
    let name = Paragraph::new(board.name.clone())
        .style(bold_highlight())
        .block(name_config.block());
    frame.render_widget(name, area);
}

fn render_board_description_field(
    app: &App,
    board: &kanban_domain::Board,
    frame: &mut Frame,
    area: Rect,
) {
    let desc_config = FieldSectionConfig::new("Description")
        .with_focus_indicator("Description [2]")
        .focused(app.focus.board_focus == BoardFocus::Description);
    let desc_lines = if let Some(desc_text) = &board.description {
        crate::markdown_renderer::render_markdown(desc_text)
    } else {
        vec![Line::from(Span::styled("No description", label_text()))]
    };
    let desc = Paragraph::new(desc_lines).block(desc_config.block());
    frame.render_widget(desc, area);
}

fn render_board_settings_section(
    app: &App,
    board: &kanban_domain::Board,
    frame: &mut Frame,
    area: Rect,
) {
    let settings_config = FieldSectionConfig::new("Settings")
        .with_focus_indicator("Settings [3]")
        .focused(app.focus.board_focus == BoardFocus::Settings);

    let mut settings_lines = vec![
        if let Some(prefix) = &board.sprint_prefix {
            metadata_line_styled("Sprint Prefix", prefix, active_item())
        } else {
            Line::from(vec![
                Span::styled("Sprint Prefix: ", label_text()),
                Span::styled(
                    app.app_config.effective_default_sprint_prefix().to_string(),
                    normal_text(),
                ),
                Span::styled(" (default)", label_text()),
            ])
        },
        if let Some(prefix) = &board.card_prefix {
            metadata_line_styled("Card Prefix", prefix, active_item())
        } else {
            Line::from(vec![
                Span::styled("Card Prefix: ", label_text()),
                Span::styled(
                    app.app_config.effective_default_card_prefix().to_string(),
                    normal_text(),
                ),
                Span::styled(" (default)", label_text()),
            ])
        },
    ];

    if let Some(sprint_prefix) =
        kanban_domain::get_active_sprint_card_prefix_override(board, app.model.sprints())
    {
        settings_lines.push(metadata_line_styled(
            "Active Sprint Card Prefix",
            sprint_prefix,
            Style::default().fg(Color::Cyan),
        ));
    }

    settings_lines.push(Line::from(""));
    settings_lines.push(metadata_line(
        "Sprint Duration",
        board
            .sprint_duration_days
            .map(|d| format!("{} days", d))
            .unwrap_or_else(|| "(not set)".to_string()),
    ));

    let available_names: Vec<&str> = board
        .sprint_names
        .iter()
        .skip(board.sprint_name_used_count)
        .map(|s| s.as_str())
        .collect();

    if !available_names.is_empty() {
        settings_lines.push(metadata_line("Sprint Names", available_names.join(", ")));
    }

    let settings = Paragraph::new(settings_lines).block(settings_config.block());
    frame.render_widget(settings, area);
}

fn render_board_sprints_list(
    app: &App,
    board: &kanban_domain::Board,
    frame: &mut Frame,
    area: Rect,
) {
    use crate::theme::colors::SELECTED_BG;

    let sprints_config = FieldSectionConfig::new("Sprints")
        .with_focus_indicator("Sprints [4]")
        .focused(app.focus.board_focus == BoardFocus::Sprints);

    let board_sprints: Vec<&Sprint> = app
        .model
        .sprints()
        .iter()
        .filter(|s| s.board_id == board.id)
        .collect();

    let mut sprint_lines = vec![];

    if board_sprints.is_empty() {
        sprint_lines.push(Line::from(Span::styled(
            "  No sprints yet. Press 'n' to create one!",
            label_text(),
        )));
    } else {
        let all_cards = app.model.live_cards();
        for (sprint_idx, sprint) in board_sprints.iter().enumerate() {
            let is_selected = app.selection.sprint.get() == Some(sprint_idx);
            let is_focused = app.focus.board_focus == BoardFocus::Sprints;

            let status_symbol = match sprint.status {
                SprintStatus::Planning => "○",
                SprintStatus::Active => "●",
                SprintStatus::Completed => "✓",
                SprintStatus::Cancelled => "✗",
            };

            let sprint_name = sprint.formatted_name(board, None);

            let card_count = all_cards
                .iter()
                .filter(|c| c.sprint_id == Some(sprint.id))
                .count();

            let is_active_sprint = board.active_sprint_id == Some(sprint.id);
            let is_ended = sprint.is_ended(chrono::Utc::now());

            let mut base_style = normal_text();
            if is_selected && is_focused {
                base_style = base_style.bg(SELECTED_BG);
            }

            let mut spans = vec![
                Span::styled(
                    format!("{} ", status_symbol),
                    sprint_status_style(sprint.status),
                ),
                Span::styled(sprint_name, base_style),
                Span::styled(format!(" ({})", card_count), label_text()),
            ];

            if is_active_sprint {
                let mut active_style = active_item();
                if is_selected && is_focused {
                    active_style = active_style.bg(SELECTED_BG);
                }
                spans.push(Span::styled(" Active", active_style));
            }

            if is_ended {
                let mut ended_style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
                if is_selected && is_focused {
                    ended_style = ended_style.bg(SELECTED_BG);
                }
                spans.push(Span::styled(" Ended", ended_style));
            }

            sprint_lines.push(Line::from(spans));
        }
    }

    let selected_idx = app.selection.sprint.get().unwrap_or(0);
    let viewport_height = area.height.saturating_sub(2) as usize;
    let scroll = scroll_offset_to_keep_visible(
        app.selection.sprint_scroll.get(),
        selected_idx,
        viewport_height,
    );
    app.selection.sprint_scroll.set(scroll);
    let sprints = Paragraph::new(sprint_lines)
        .block(sprints_config.block())
        .scroll((scroll as u16, 0));
    frame.render_widget(sprints, area);
}

fn render_board_columns_list(
    app: &mut App,
    board: &kanban_domain::Board,
    frame: &mut Frame,
    area: Rect,
) {
    use crate::theme::colors::SELECTED_BG;

    let columns_config = FieldSectionConfig::new("Columns")
        .with_focus_indicator("Columns [5]")
        .focused(app.focus.board_focus == BoardFocus::Columns);

    let total_columns = sorted_board_columns(board.id, app.model.columns()).len();
    let board_columns = app.visible_board_columns(board.id);

    let mut column_lines = vec![];

    if board_columns.is_empty() {
        column_lines.push(Line::from(Span::styled(
            columns_empty_state_message(total_columns),
            label_text(),
        )));
    } else {
        let all_cards = app.model.live_cards();
        let is_focused = app.focus.board_focus == BoardFocus::Columns;
        let viewport_height = area.height.saturating_sub(2) as usize;

        // Refreshed here rather than relying on a handler having run first:
        // jumping straight to the Columns panel (key '5') sets focus without
        // touching the list.
        app.dialog_input
            .column_list
            .update_item_count(board_columns.len());
        app.dialog_input
            .column_list
            .ensure_selected_visible(viewport_height);
        let render_info = app
            .dialog_input
            .column_list
            .get_render_info(viewport_height);
        let selected_idx = app.dialog_input.column_list.get_selected_index();

        for &column_idx in &render_info.visible_indices {
            let Some(column) = board_columns.get(column_idx) else {
                continue;
            };
            let is_selected = selected_idx == Some(column_idx);

            let card_count = all_cards
                .iter()
                .filter(|c| c.column_id == column.id)
                .count();

            let mut base_style = normal_text();
            if is_selected && is_focused {
                base_style = base_style.bg(SELECTED_BG);
            }

            let mut spans = vec![
                Span::styled(format!("{}. ", column.position + 1), label_text()),
                Span::styled(&column.name, base_style),
                Span::styled(format!(" ({})", card_count), label_text()),
            ];
            if let Some(status) = column.default_status {
                spans.push(Span::styled(
                    format!(
                        " [{}]",
                        kanban_view::selection_dialog::default_status_label(Some(status))
                    ),
                    label_text(),
                ));
            }

            column_lines.push(Line::from(spans));
        }
    }

    let columns = Paragraph::new(column_lines).block(columns_config.block());
    frame.render_widget(columns, area);
}

fn columns_empty_state_message(total_columns: usize) -> &'static str {
    if total_columns == 0 {
        "  No columns yet. Press 'n' to create one!"
    } else {
        "  No columns match search"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_columns_empty_state_message_when_board_has_no_columns() {
        assert_eq!(
            columns_empty_state_message(0),
            "  No columns yet. Press 'n' to create one!"
        );
    }

    #[test]
    fn test_columns_empty_state_message_when_search_matches_nothing() {
        assert_eq!(columns_empty_state_message(3), "  No columns match search");
    }
}
