use crate::app::App;
use crate::components::*;
use crate::theme::*;
use kanban_domain::{Sprint, SprintStatus};
use kanban_view::model::Model;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub(super) fn render_sprint_detail_view(app: &mut App, frame: &mut Frame, area: Rect) {
    let sprint_idx = match app.selection.active_sprint_index {
        Some(i) => i,
        None => return,
    };
    let sprint = match app.model.sprints().get(sprint_idx).cloned() {
        Some(s) => s,
        None => return,
    };
    let board = match app.active_board().cloned() {
        Some(b) => b,
        None => return,
    };

    if sprint.status == SprintStatus::Completed {
        render_sprint_detail_with_tasks(app, frame, area, &sprint, &board);
    } else {
        render_sprint_detail_metadata(app, frame, area, &sprint, &board);
    }
}

fn render_sprint_detail_metadata(
    app: &mut App,
    frame: &mut Frame,
    area: Rect,
    sprint: &Sprint,
    board: &kanban_domain::Board,
) {
    let mut lines = vec![];
    lines.extend(sprint_header_lines(sprint, board));
    lines.extend(sprint_date_lines(sprint));
    lines.push(Line::from(""));
    lines.extend(sprint_card_assignment_lines(app, sprint, board));
    lines.push(Line::from(""));
    lines.extend(sprint_prefix_lines(sprint));
    lines.extend(sprint_timestamp_lines(sprint));

    let content = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(focused_border())
            .title("Sprint Details"),
    );
    frame.render_widget(content, area);
}

fn sprint_header_lines(sprint: &Sprint, board: &kanban_domain::Board) -> Vec<Line<'static>> {
    let sprint_name = sprint.formatted_name(board, "sprint");
    let mut lines = vec![
        metadata_line_styled("Sprint", sprint_name, bold_highlight()),
        Line::from(""),
        metadata_line_styled(
            "Status",
            format!("{:?}", sprint.status),
            sprint_status_style(sprint.status),
        ),
        metadata_line("Sprint Number", sprint.sprint_number.to_string()),
    ];
    if let Some(name) = sprint.get_name(board) {
        lines.push(metadata_line("Name", name.to_string()));
    }
    lines
}

fn sprint_date_lines(sprint: &Sprint) -> Vec<Line<'static>> {
    let mut lines = vec![];
    if let Some(start) = sprint.start_date {
        lines.push(metadata_line(
            "Start Date",
            start.format("%Y-%m-%d %H:%M UTC").to_string(),
        ));
    }
    if let Some(end) = sprint.end_date {
        let end_style = if sprint.is_ended(chrono::Utc::now()) {
            Style::default().fg(Color::Red)
        } else {
            normal_text()
        };
        lines.push(metadata_line_styled(
            "End Date",
            end.format("%Y-%m-%d %H:%M UTC").to_string(),
            end_style,
        ));
    }
    lines
}

fn sprint_card_assignment_lines(
    app: &App,
    sprint: &Sprint,
    board: &kanban_domain::Board,
) -> Vec<Line<'static>> {
    let card_count = app
        .model
        .live_cards()
        .iter()
        .filter(|c| c.sprint_id == Some(sprint.id))
        .count();
    let mut lines = vec![metadata_line_styled(
        "Cards Assigned",
        card_count.to_string(),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];
    if board.active_sprint_id == Some(sprint.id) {
        lines.push(metadata_line_styled("Active Sprint", "Yes", active_item()));
    }
    lines
}

fn sprint_prefix_lines(sprint: &Sprint) -> Vec<Line<'static>> {
    let mut lines = vec![];
    if let Some(prefix) = &sprint.prefix {
        lines.push(metadata_line_styled(
            "Sprint Prefix",
            prefix.clone(),
            active_item(),
        ));
    }
    if let Some(prefix) = &sprint.card_prefix {
        lines.push(metadata_line_styled(
            "Card Prefix Override",
            prefix.clone(),
            active_item(),
        ));
    }
    if sprint.prefix.is_some() || sprint.card_prefix.is_some() {
        lines.push(Line::from(""));
    }
    lines
}

fn sprint_timestamp_lines(sprint: &Sprint) -> Vec<Line<'static>> {
    vec![
        metadata_line_styled(
            "Created",
            sprint.created_at.format("%Y-%m-%d %H:%M UTC").to_string(),
            label_text(),
        ),
        metadata_line_styled(
            "Updated",
            sprint.updated_at.format("%Y-%m-%d %H:%M UTC").to_string(),
            label_text(),
        ),
    ]
}

pub(super) fn render_sprint_detail_with_tasks(
    app: &mut App,
    frame: &mut Frame,
    area: Rect,
    sprint: &Sprint,
    board: &kanban_domain::Board,
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let uncompleted_focused = app.sprint_view.panel == crate::app::SprintTaskPanel::Uncompleted;
    let completed_focused = app.sprint_view.panel == crate::app::SprintTaskPanel::Completed;

    let uncompleted_viewport = chunks[0].height.saturating_sub(2) as usize;
    let completed_viewport = chunks[1].height.saturating_sub(2) as usize;
    app.sprint_view
        .sync_scroll(uncompleted_viewport, completed_viewport);

    render_sprint_task_panel_with_selection(
        app,
        frame,
        chunks[0],
        sprint,
        board,
        &app.sprint_view.uncompleted_cards,
        "Uncompleted",
        uncompleted_focused,
    );

    render_sprint_task_panel_with_selection(
        app,
        frame,
        chunks[1],
        sprint,
        board,
        &app.sprint_view.completed_cards,
        "Completed",
        completed_focused,
    );
}

fn calculate_task_panel_points(task_list: &kanban_view::card_list::CardList, model: &Model) -> u32 {
    let filtered: Vec<&kanban_domain::Card> = task_list
        .cards
        .iter()
        .filter_map(|card_id| model.card_by_id(*card_id))
        .collect();
    kanban_domain::calculate_points(&filtered)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_sprint_task_panel_with_selection(
    app: &App,
    frame: &mut Frame,
    area: Rect,
    _sprint: &Sprint,
    board: &kanban_domain::Board,
    task_list: &kanban_view::card_list::CardList,
    title_suffix: &str,
    is_focused: bool,
) {
    let mut lines = vec![];
    let selected_idx = task_list.get_selected_index();

    if task_list.is_empty() {
        lines.push(Line::from(Span::styled("  (no tasks)", label_text())));
    } else {
        let viewport_height = area.height.saturating_sub(2) as usize;
        let render_info = task_list.get_render_info(viewport_height);

        lines.extend(crate::scroll_indicators::render_above_indicator(
            render_info.show_above_indicator,
            render_info.cards_above_count,
            "Task",
        ));

        let sprints = &app.model.sprints();

        for card_idx in &render_info.visible_card_indices {
            if let Some(card_id) = task_list.cards.get(*card_idx) {
                if let Some(card) = app.model.card_by_id(*card_id) {
                    let is_selected = selected_idx == Some(*card_idx) && is_focused;
                    let animation_type = app
                        .animation
                        .animating
                        .get(&card.id)
                        .map(|a| a.animation_type);
                    let line = render_card_list_item(CardListItemConfig {
                        card,
                        board,
                        sprints,
                        is_selected,
                        is_focused,
                        is_multi_selected: false,
                        show_sprint_name: false,
                        animation_type,
                        search_query: None,
                    });
                    lines.push(line);
                }
            }
        }

        lines.extend(crate::scroll_indicators::render_below_indicator(
            render_info.show_below_indicator,
            render_info.cards_below_count,
            "Task",
        ));
    }

    let points = calculate_task_panel_points(task_list, &app.model);

    lines.push(Line::from(Span::styled(
        format!("Points: {}", points),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));

    let border_style = if is_focused {
        focused_border()
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let content = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(format!("{} ({})", title_suffix, task_list.len())),
    );
    frame.render_widget(content, area);
}
