use crate::app::App;
use crate::components::*;
use crate::theme::*;
use ratatui::{widgets::ListItem, Frame};

pub(crate) fn render_create_column_popup(app: &App, frame: &mut Frame) {
    render_input_popup(
        frame,
        "Create New Column",
        "Column Name:",
        app.input.as_str(),
        app.input.cursor_byte_offset(),
    );
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
