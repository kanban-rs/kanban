mod helpers;

use kanban_domain::KanbanOperations;
use kanban_tui::app::focus::{BoardFocus, SettingsFocus};
use kanban_tui::app::mode::AppMode;
use kanban_tui::App;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use ratatui::Terminal;

fn render_settings(app: &App) -> Buffer {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render_settings_view(app, frame, frame.area());
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn render_full(app: &mut App) -> Buffer {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render(app, frame);
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn find_cell_position(buffer: &Buffer, needle: &str) -> (u16, u16) {
    for y in 0..buffer.area.height {
        let mut row = String::new();
        for x in 0..buffer.area.width {
            row.push_str(buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
        if let Some(byte_idx) = row.find(needle) {
            let x = row[..byte_idx].chars().count() as u16;
            return (x, y);
        }
    }
    panic!("text {needle:?} not found in buffer");
}

fn configured_rgb() -> Color {
    Color::Rgb(0x2E, 0x34, 0x40)
}

fn seed_board_with_sprint_and_column() -> (App, uuid::Uuid) {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    app.ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    app.ctx
        .create_sprint(board.id, None, Some("Sprint 1".to_string()))
        .unwrap();
    app.selection.active_board_id = Some(board.id);
    app.reload_model();
    app.prepare_frame();
    (app, board.id)
}

#[test]
fn test_settings_selected_row_uses_terminal_blue_when_nothing_is_configured() {
    let app = helpers::setup_settings_app();
    let buffer = render_settings(&app);
    let (x, y) = find_cell_position(&buffer, "Configuration Format: ");
    let cell = buffer.cell((x, y)).unwrap();
    assert_eq!(cell.style().bg, Some(Color::Blue));
}

#[test]
fn test_settings_selected_row_uses_the_configured_background_highlight() {
    let mut app = helpers::setup_settings_app();
    app.app_config.background_highlight = Some("#2E3440".to_string());
    let buffer = render_settings(&app);
    let (label_x, y) = find_cell_position(&buffer, "Configuration Format: ");
    let label_cell = buffer.cell((label_x, y)).unwrap();
    assert_eq!(label_cell.style().bg, Some(configured_rgb()));
    assert_eq!(label_cell.style().fg, Some(Color::DarkGray));

    let (value_x, _) = find_cell_position(&buffer, "Configuration Format: ");
    let value_x = value_x + "Configuration Format: ".chars().count() as u16;
    let value_cell = buffer.cell((value_x, y)).unwrap();
    assert_eq!(value_cell.style().bg, Some(configured_rgb()));
    assert_eq!(value_cell.style().fg, Some(Color::White));
}

#[test]
fn test_settings_unselected_row_has_no_highlight_background() {
    let mut app = helpers::setup_settings_app();
    app.app_config.background_highlight = Some("#2E3440".to_string());
    let buffer = render_settings(&app);
    let (x, y) = find_cell_position(&buffer, "Configuration Location: ");
    let cell = buffer.cell((x, y)).unwrap();
    assert_ne!(cell.style().bg, Some(configured_rgb()));
}

#[test]
fn test_settings_export_button_uses_the_configured_background_highlight() {
    let mut app = helpers::setup_settings_app();
    app.app_config.background_highlight = Some("#2E3440".to_string());
    app.focus.settings_focus = SettingsFocus::Storage;
    app.selection.settings_storage.set(Some(3));
    let buffer = render_settings(&app);
    let (x, y) = find_cell_position(&buffer, "Export Boards");
    let cell = buffer.cell((x, y)).unwrap();
    assert_eq!(cell.style().bg, Some(configured_rgb()));
    assert_eq!(cell.style().fg, Some(Color::White));
}

#[test]
fn test_board_detail_selected_sprint_uses_the_configured_background_highlight() {
    let (mut app, _board_id) = seed_board_with_sprint_and_column();
    app.app_config.background_highlight = Some("#2E3440".to_string());
    app.push_mode(AppMode::BoardDetail);
    app.focus.board_focus = BoardFocus::Sprints;
    app.selection.sprint.set(Some(0));

    let buffer = render_full(&mut app);
    let (x, y) = find_cell_position(&buffer, "Sprint 1");
    let cell = buffer.cell((x, y)).unwrap();
    assert_eq!(cell.style().bg, Some(configured_rgb()));
}

#[test]
fn test_board_detail_selected_column_uses_the_configured_background_highlight() {
    let (mut app, _board_id) = seed_board_with_sprint_and_column();
    app.app_config.background_highlight = Some("#2E3440".to_string());
    app.push_mode(AppMode::BoardDetail);
    app.focus.board_focus = BoardFocus::Columns;

    let buffer = render_full(&mut app);
    let (x, y) = find_cell_position(&buffer, "Todo");
    let cell = buffer.cell((x, y)).unwrap();
    assert_eq!(cell.style().bg, Some(configured_rgb()));
}
