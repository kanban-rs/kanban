use ratatui::{Frame, layout::{Constraint, Direction, Layout, Rect}, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::{Block, Borders, Paragraph, Clear}};
use crate::app::{App, AppMode, Focus, CardFocus, BoardFocus};
use kanban_domain::CardStatus;

pub fn render(app: &App, frame: &mut Frame) {
    match app.mode {
        AppMode::CardDetail => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(3),
                ])
                .split(frame.area());

            render_header(frame, chunks[0]);
            render_card_detail_view(app, frame, chunks[1]);
            render_footer(app, frame, chunks[2]);
        }
        AppMode::BoardDetail => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(3),
                ])
                .split(frame.area());

            render_header(frame, chunks[0]);
            render_board_detail_view(app, frame, chunks[1]);
            render_footer(app, frame, chunks[2]);
        }
        _ => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(3),
                ])
                .split(frame.area());

            render_header(frame, chunks[0]);
            render_main(app, frame, chunks[1]);
            render_footer(app, frame, chunks[2]);

            match app.mode {
                AppMode::CreateBoard => render_create_board_popup(app, frame),
                AppMode::CreateCard => render_create_card_popup(app, frame),
                AppMode::RenameBoard => render_rename_board_popup(app, frame),
                AppMode::ExportBoard => render_export_board_popup(app, frame),
                AppMode::ExportAll => render_export_all_popup(app, frame),
                AppMode::ImportBoard => render_import_board_popup(app, frame),
                AppMode::SetCardPoints => render_set_card_points_popup(app, frame),
                _ => {}
            }
        }
    }
}

fn render_header(frame: &mut Frame, area: Rect) {
    let title = Paragraph::new("Kanban Board")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, area);
}

fn render_main(app: &App, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    render_projects_panel(app, frame, chunks[0]);
    render_tasks_panel(app, frame, chunks[1]);
}

fn render_projects_panel(app: &App, frame: &mut Frame, area: Rect) {
    let mut lines = vec![
        Line::from(Span::styled(
            "Projects",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::raw("")),
    ];

    if app.boards.is_empty() {
        lines.push(Line::from(Span::styled(
            "No projects yet. Press 'n' to create one!",
            Style::default().fg(Color::Gray),
        )));
    } else {
        for (idx, board) in app.boards.iter().enumerate() {
            let is_selected = app.board_selection.get() == Some(idx);
            let is_active = app.active_board_index == Some(idx);
            let is_focused = app.focus == Focus::Projects;

            let mut style = Style::default();
            let prefix;

            if is_active {
                style = style.fg(Color::Green).add_modifier(Modifier::BOLD);
                prefix = "● ";
            } else {
                style = style.fg(Color::White);
                prefix = "  ";
            }

            if is_selected && is_focused {
                style = style.bg(Color::Blue);
            }

            lines.push(Line::from(Span::styled(
                format!("{}{}", prefix, board.name),
                style,
            )));
        }
    }

    let is_focused = app.focus == Focus::Projects;
    let border_color = if is_focused { Color::Cyan } else { Color::White };
    let title = if is_focused { "Projects [1]" } else { "Projects" };

    let content = Paragraph::new(lines)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(title));
    frame.render_widget(content, area);
}

fn render_tasks_panel(app: &App, frame: &mut Frame, area: Rect) {
    let board_idx = app.active_board_index.or(app.board_selection.get());

    let project_name = if let Some(idx) = board_idx {
        app.boards.get(idx).map(|b| b.name.as_str()).unwrap_or("Unknown")
    } else {
        "No Project"
    };

    let mut lines = vec![
        Line::from(Span::styled(
            format!("{}", project_name),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::raw("")),
    ];

    if let Some(idx) = board_idx {
        if let Some(board) = app.boards.get(idx) {
            let board_tasks: Vec<_> = app.cards.iter()
                .filter(|card| {
                    app.columns.iter()
                        .any(|col| col.id == card.column_id && col.board_id == board.id)
                })
                .collect();

            if board_tasks.is_empty() {
                let message = if app.active_board_index.is_some() {
                    "  No tasks yet. Press 'n' to create one!"
                } else {
                    "  (Enter/Space) to add tasks"
                };
                lines.push(Line::from(Span::styled(
                    message,
                    Style::default().fg(Color::Gray),
                )));
            } else {
                for (task_idx, task) in board_tasks.iter().enumerate() {
                    let is_selected = app.card_selection.get() == Some(task_idx);
                    let is_focused = app.focus == Focus::Tasks;
                    let is_done = task.status == CardStatus::Done;

                    let (checkbox, text_color, text_modifier) = if is_done {
                        ("☑", Color::DarkGray, Modifier::CROSSED_OUT)
                    } else {
                        ("☐", Color::White, Modifier::empty())
                    };

                    let mut style = Style::default()
                        .fg(text_color)
                        .add_modifier(text_modifier);

                    if is_selected && is_focused {
                        style = style.bg(Color::Blue);
                    }

                    let points_badge = if let Some(points) = task.points {
                        format!(" [{}]", points)
                    } else {
                        String::new()
                    };

                    let line = if points_badge.is_empty() {
                        Line::from(Span::styled(
                            format!("  {} {}", checkbox, task.title),
                            style,
                        ))
                    } else {
                        let points_color = task.points.map(|p| match p {
                            1 => Color::Cyan,
                            2 => Color::Green,
                            3 => Color::Yellow,
                            4 => Color::LightMagenta,
                            5 => Color::Red,
                            _ => Color::White,
                        }).unwrap_or(Color::White);

                        let mut points_style = Style::default().fg(points_color);
                        if is_selected && is_focused {
                            points_style = points_style.bg(Color::Blue);
                        }

                        Line::from(vec![
                            Span::styled(format!("  {} {}", checkbox, task.title), style),
                            Span::styled(points_badge, points_style),
                        ])
                    };

                    lines.push(line);
                }
            }
        }
    } else {
        lines.push(Line::from(Span::styled(
            "  Select a project to preview tasks",
            Style::default().fg(Color::Gray),
        )));
    }

    let is_focused = app.focus == Focus::Tasks;
    let border_color = if is_focused { Color::Cyan } else { Color::White };
    let title = if is_focused { "Tasks [2]" } else { "Tasks" };

    let content = Paragraph::new(lines)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(title));
    frame.render_widget(content, area);
}

fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    let help_text = match app.mode {
        AppMode::Normal => "q: quit | n: new | r: rename | e: edit board | x: export | X: export all | i: import | c: toggle complete | 1/2: switch panel | j/k: navigate | Enter/Space: activate",
        AppMode::CreateBoard => "ESC: cancel | ENTER: confirm",
        AppMode::CreateCard => "ESC: cancel | ENTER: confirm",
        AppMode::RenameBoard => "ESC: cancel | ENTER: confirm",
        AppMode::ExportBoard => "ESC: cancel | ENTER: export",
        AppMode::ExportAll => "ESC: cancel | ENTER: export all",
        AppMode::ImportBoard => "ESC: cancel | j/k: navigate | ENTER/Space: import selected",
        AppMode::CardDetail => match app.card_focus {
            CardFocus::Title => "q: quit | ESC: back | 1/2/3: select panel | e: edit title",
            CardFocus::Description => "q: quit | ESC: back | 1/2/3: select panel | e: edit description",
            CardFocus::Metadata => "q: quit | ESC: back | 1/2/3: select panel | e: edit points",
        },
        AppMode::SetCardPoints => "ESC: cancel | ENTER: confirm",
        AppMode::BoardDetail => match app.board_focus {
            BoardFocus::Name => "q: quit | ESC: back | 1/2: select panel | e: edit name",
            BoardFocus::Description => "q: quit | ESC: back | 1/2: select panel | e: edit description",
        },
    };
    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, area);
}

fn render_create_board_popup(app: &App, frame: &mut Frame) {
    render_input_popup(app, frame, "Create New Project", "Project Name:");
}

fn render_create_card_popup(app: &App, frame: &mut Frame) {
    render_input_popup(app, frame, "Create New Task", "Task Title:");
}

fn render_set_card_points_popup(app: &App, frame: &mut Frame) {
    render_input_popup(app, frame, "Set Points", "Points (1-5 or empty):");
}

fn render_card_detail_view(app: &App, frame: &mut Frame, area: Rect) {
    if let Some(task_idx) = app.active_card_index {
        if let Some(board_idx) = app.active_board_index {
            if let Some(board) = app.boards.get(board_idx) {
                let board_tasks: Vec<_> = app.cards.iter()
                    .filter(|card| {
                        app.columns.iter()
                            .any(|col| col.id == card.column_id && col.board_id == board.id)
                    })
                    .collect();

                if let Some(task) = board_tasks.get(task_idx) {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(5),
                            Constraint::Length(5),
                            Constraint::Min(0),
                        ])
                        .split(area);

                    let title_focused = app.card_focus == CardFocus::Title;
                    let title_border_color = if title_focused { Color::Cyan } else { Color::White };
                    let title_block = Block::default()
                        .title(if title_focused { "Task Title [1]" } else { "Task Title" })
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(title_border_color));
                    let title = Paragraph::new(task.title.clone())
                        .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
                        .block(title_block);
                    frame.render_widget(title, chunks[0]);

                    let meta_focused = app.card_focus == CardFocus::Metadata;
                    let meta_border_color = if meta_focused { Color::Cyan } else { Color::White };
                    let meta_block = Block::default()
                        .title(if meta_focused { "Metadata [2]" } else { "Metadata" })
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(meta_border_color));
                    let meta_lines = vec![
                        Line::from(vec![
                            Span::styled("Priority: ", Style::default().fg(Color::Gray)),
                            Span::styled(format!("{:?}", task.priority), Style::default().fg(Color::White)),
                            Span::raw("  "),
                            Span::styled("Status: ", Style::default().fg(Color::Gray)),
                            Span::styled(format!("{:?}", task.status), Style::default().fg(Color::White)),
                            Span::raw("  "),
                            Span::styled("Points: ", Style::default().fg(Color::Gray)),
                            Span::styled(
                                task.points.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string()),
                                Style::default().fg(Color::White)
                            ),
                        ]),
                        Line::from(if let Some(due_date) = task.due_date {
                            vec![
                                Span::styled("Due: ", Style::default().fg(Color::Gray)),
                                Span::styled(due_date.format("%Y-%m-%d %H:%M").to_string(), Style::default().fg(Color::Red)),
                            ]
                        } else {
                            vec![Span::styled("No due date", Style::default().fg(Color::Gray))]
                        }),
                    ];
                    let meta = Paragraph::new(meta_lines).block(meta_block);
                    frame.render_widget(meta, chunks[1]);

                    let desc_focused = app.card_focus == CardFocus::Description;
                    let desc_border_color = if desc_focused { Color::Cyan } else { Color::White };
                    let desc_block = Block::default()
                        .title(if desc_focused { "Description [3]" } else { "Description" })
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(desc_border_color));
                    let desc_text = if let Some(desc) = &task.description {
                        desc.clone()
                    } else {
                        "No description".to_string()
                    };
                    let desc = Paragraph::new(desc_text)
                        .style(Style::default().fg(Color::White))
                        .block(desc_block);
                    frame.render_widget(desc, chunks[2]);
                }
            }
        }
    }
}

fn render_rename_board_popup(app: &App, frame: &mut Frame) {
    render_input_popup(app, frame, "Rename Project", "Project Name:");
}

fn render_export_board_popup(app: &App, frame: &mut Frame) {
    render_input_popup(app, frame, "Export Board", "Filename:");
}

fn render_export_all_popup(app: &App, frame: &mut Frame) {
    render_input_popup(app, frame, "Export All Boards", "Filename:");
}

fn render_import_board_popup(app: &App, frame: &mut Frame) {
    let area = centered_rect(60, 50, frame.area());

    frame.render_widget(Clear, area);

    let block = Block::default()
        .title("Import Board")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    let label = Paragraph::new("Select a JSON file to import:")
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(label, chunks[0]);

    if app.import_files.is_empty() {
        let empty_msg = Paragraph::new("No JSON files found in current directory")
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(empty_msg, chunks[1]);
    } else {
        let mut lines = vec![];
        for (idx, filename) in app.import_files.iter().enumerate() {
            let is_selected = app.import_selection.get() == Some(idx);
            let style = if is_selected {
                Style::default().fg(Color::White).bg(Color::Blue)
            } else {
                Style::default().fg(Color::White)
            };
            let prefix = if is_selected { "> " } else { "  " };
            lines.push(Line::from(Span::styled(format!("{}{}", prefix, filename), style)));
        }
        let list = Paragraph::new(lines);
        frame.render_widget(list, chunks[1]);
    }
}

fn render_board_detail_view(app: &App, frame: &mut Frame, area: Rect) {
    if let Some(board_idx) = app.board_selection.get() {
        if let Some(board) = app.boards.get(board_idx) {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(5),
                    Constraint::Min(0),
                ])
                .split(area);

            let name_focused = app.board_focus == BoardFocus::Name;
            let name_border_color = if name_focused { Color::Cyan } else { Color::White };
            let name_block = Block::default()
                .title(if name_focused { "Board Name [1]" } else { "Board Name" })
                .borders(Borders::ALL)
                .border_style(Style::default().fg(name_border_color));
            let name = Paragraph::new(board.name.clone())
                .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
                .block(name_block);
            frame.render_widget(name, chunks[0]);

            let desc_focused = app.board_focus == BoardFocus::Description;
            let desc_border_color = if desc_focused { Color::Cyan } else { Color::White };
            let desc_block = Block::default()
                .title(if desc_focused { "Description [2]" } else { "Description" })
                .borders(Borders::ALL)
                .border_style(Style::default().fg(desc_border_color));
            let desc_text = if let Some(desc) = &board.description {
                desc.clone()
            } else {
                "No description".to_string()
            };
            let desc = Paragraph::new(desc_text)
                .style(Style::default().fg(Color::White))
                .block(desc_block);
            frame.render_widget(desc, chunks[1]);
        }
    }
}

fn render_input_popup(app: &App, frame: &mut Frame, title: &str, label: &str) {
    let area = centered_rect(60, 30, frame.area());

    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(inner);

    let label_widget = Paragraph::new(label)
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(label_widget, chunks[0]);

    let input = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(input, chunks[1]);

    let cursor_x = chunks[1].x + app.input.cursor_pos() as u16 + 1;
    let cursor_y = chunks[1].y + 1;
    frame.set_cursor_position((cursor_x, cursor_y));
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
