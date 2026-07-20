use crate::app::App;
use crate::components::*;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub(crate) fn render_create_card_popup(app: &App, frame: &mut Frame) {
    use crate::components::centered_rect;

    let Some(board) = app.active_board() else {
        render_input_popup(
            frame,
            "Create New Task",
            "Task Title:",
            app.input.as_str(),
            app.input.cursor_byte_offset(),
        );
        return;
    };

    let area = centered_rect(60, 60, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title("Create New Task")
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

    let title_focused = app.dialog_input.create_card_focus_is_title();
    let unfocused_border = Style::default().fg(Color::DarkGray);

    frame.render_widget(
        Paragraph::new("Task Title:").style(Style::default().fg(Color::Yellow)),
        chunks[0],
    );

    let input = Paragraph::new(app.input.as_str())
        .style(crate::theme::normal_text())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if title_focused {
                    crate::theme::focused_border()
                } else {
                    unfocused_border
                }),
        );
    frame.render_widget(input, chunks[1]);
    if title_focused {
        let cursor_x = chunks[1].x + app.input.cursor_byte_offset() as u16 + 1;
        let cursor_y = chunks[1].y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }

    frame.render_widget(
        Paragraph::new("Sprint:").style(Style::default().fg(Color::Yellow)),
        chunks[2],
    );

    let picker_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if title_focused {
            unfocused_border
        } else {
            crate::theme::focused_border()
        });
    let picker_inner = picker_block.inner(chunks[3]);
    frame.render_widget(picker_block, chunks[3]);
    app.dialog_input.create_card_sprint_picker.render(
        frame,
        picker_inner,
        app.model.sprints(),
        board,
        chrono::Utc::now(),
    );
}

pub(crate) fn render_set_card_points_popup(app: &App, frame: &mut Frame) {
    render_input_popup(
        frame,
        "Set Points",
        "Points (1-5 or empty):",
        app.input.as_str(),
        app.input.cursor_byte_offset(),
    );
}

pub(crate) fn render_set_card_priority_popup(app: &App, frame: &mut Frame) {
    use crate::components::{PriorityDialog, SelectionDialog};
    let dialog = PriorityDialog;
    dialog.render(app, frame);
}

pub(crate) fn render_set_multiple_cards_priority_popup(app: &App, frame: &mut Frame) {
    use crate::components::{BulkPriorityDialog, SelectionDialog};
    let dialog = BulkPriorityDialog {
        count: app.multi_select.selected_cards.len(),
    };
    dialog.render(app, frame);
}

pub(crate) fn render_order_cards_popup(app: &App, frame: &mut Frame) {
    use crate::components::{SelectionDialog, SortFieldDialog};
    let dialog = SortFieldDialog;
    dialog.render(app, frame);
}

pub(crate) fn render_assign_sprint_popup(app: &App, frame: &mut Frame) {
    use crate::components::{SelectionDialog, SprintAssignDialog};
    let dialog = SprintAssignDialog;
    dialog.render(app, frame);
}

pub(crate) fn render_assign_multiple_cards_popup(app: &App, frame: &mut Frame) {
    let area = centered_rect(60, 50, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(
            "Assign {} Cards to Sprint",
            app.multi_select.selected_cards.len()
        ))
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    frame.render_widget(
        Paragraph::new("Select sprint:").style(Style::default().fg(Color::Yellow)),
        chunks[0],
    );

    let Some(board) = app.active_board() else {
        return;
    };
    app.dialog_input.assign_sprint_picker.render(
        frame,
        chunks[1],
        app.model.sprints(),
        board,
        chrono::Utc::now(),
    );
}
