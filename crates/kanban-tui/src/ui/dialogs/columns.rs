use crate::app::App;
use crate::components::*;
use crate::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

pub(crate) fn render_create_column_popup(app: &App, frame: &mut Frame) {
    let statuses = kanban_view::selection_dialog::DEFAULT_STATUS_POPUP_ORDER;
    let dialog_height = (1 + 3 + 1 + statuses.len() + 2 + 6) as u16;
    let area = centered_rect_abs(60, dialog_height, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title("Create New Column")
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
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    let name_focused = app.dialog_input.create_column_focus_is_name();
    let unfocused_border = Style::default().fg(Color::DarkGray);

    frame.render_widget(
        Paragraph::new("Column Name:").style(Style::default().fg(Color::Yellow)),
        chunks[0],
    );

    let input = Paragraph::new(app.input.as_str())
        .style(crate::theme::normal_text())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if name_focused {
                    crate::theme::focused_border()
                } else {
                    unfocused_border
                }),
        );
    frame.render_widget(input, chunks[1]);
    if name_focused {
        let cursor_x = chunks[1].x + app.input.cursor_byte_offset() as u16 + 1;
        let cursor_y = chunks[1].y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }

    frame.render_widget(
        Paragraph::new("Default Status (Tab to switch, j/k to select):")
            .style(Style::default().fg(Color::Yellow)),
        chunks[2],
    );

    let selected = app.dialog_input.default_status_selection.get();
    let items: Vec<ListItem> = statuses
        .iter()
        .enumerate()
        .map(|(idx, (_, label))| {
            let style = if Some(idx) == selected {
                bold_highlight()
            } else {
                normal_text()
            };
            ListItem::new(*label).style(style)
        })
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).border_style(
        if name_focused {
            unfocused_border
        } else {
            crate::theme::focused_border()
        },
    ));
    frame.render_widget(list, chunks[3]);
}

pub(crate) fn render_rename_column_popup(app: &App, frame: &mut Frame) {
    render_input_popup(
        frame,
        "Rename Column",
        "New Column Name:",
        app.input.as_str(),
        app.input.cursor_byte_offset(),
    );
}

pub(crate) fn render_set_column_default_status_popup(app: &App, frame: &mut Frame) {
    use crate::components::{ColumnDefaultStatusDialog, SelectionDialog};
    let dialog = ColumnDefaultStatusDialog;
    dialog.render(app, frame);
}

pub(crate) fn render_delete_column_confirm_popup(_app: &App, frame: &mut Frame) {
    super::render_confirm_popup(
        frame,
        "Delete Column",
        "Are you sure you want to delete this column?\nAll cards will be moved to the first column."
            .to_string(),
    );
}

pub(crate) fn render_select_task_list_view_popup(app: &App, frame: &mut Frame) {
    use kanban_domain::TaskListView;

    let views = [
        TaskListView::Flat,
        TaskListView::GroupedByColumn,
        TaskListView::ColumnView,
    ];

    let selected = app.dialog_input.task_list_view_selection.get();

    let current_view = app.active_board().map(|board| board.task_list_view);

    let items: Vec<ListItem> = views
        .iter()
        .enumerate()
        .map(|(idx, view)| {
            let style = if Some(idx) == selected {
                bold_highlight()
            } else {
                normal_text()
            };
            let is_current = current_view == Some(*view);
            let view_name = match view {
                TaskListView::Flat => {
                    if is_current {
                        "Flat (current)"
                    } else {
                        "Flat"
                    }
                }
                TaskListView::GroupedByColumn => {
                    if is_current {
                        "Grouped by Column (current)"
                    } else {
                        "Grouped by Column"
                    }
                }
                TaskListView::ColumnView => {
                    if is_current {
                        "Column View (kanban board) (current)"
                    } else {
                        "Column View (kanban board)"
                    }
                }
            };
            ListItem::new(view_name).style(style)
        })
        .collect();

    render_selection_popup_with_list_items(frame, "Select Task List View", items, 50, 40);
}
