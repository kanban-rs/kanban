use crossterm::event::{KeyCode, KeyEvent};
use kanban_domain::KanbanOperations;
use kanban_tui::app::focus::Focus;
use kanban_tui::events::EventHandler;
use kanban_tui::App;
use ratatui::backend::{CrosstermBackend, TestBackend};
use ratatui::style::Color;
use ratatui::{Terminal, TerminalOptions, Viewport};
use std::io;

fn seed_two_boards() -> App {
    let mut app = App::test_default();
    app.ctx.create_board("Alpha".to_string(), None).unwrap();
    app.ctx.create_board("Beta".to_string(), None).unwrap();
    app.reload_model();
    app.prepare_frame();
    app.focus.active = Focus::Boards;
    app.board_list.inner_mut().set_selected_index(Some(0));
    app
}

fn render_buffer(app: &mut App) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render(app, frame);
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn find_row_containing(buffer: &ratatui::buffer::Buffer, needle: &str) -> u16 {
    for y in 0..buffer.area.height {
        let mut row = String::new();
        for x in 0..buffer.area.width {
            row.push_str(buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
        if row.contains(needle) {
            return y;
        }
    }
    panic!("row containing {needle:?} not found in buffer");
}

fn find_col_containing(buffer: &ratatui::buffer::Buffer, y: u16, needle: &str) -> u16 {
    for x in 0..buffer.area.width {
        if buffer
            .cell((x, y))
            .map(|c| c.symbol() == needle)
            .unwrap_or(false)
        {
            return x;
        }
    }
    panic!("column containing {needle:?} not found on row {y}");
}

#[test]
fn test_selected_project_row_uses_terminal_blue_when_nothing_is_configured() {
    let mut app = seed_two_boards();
    let buffer = render_buffer(&mut app);
    let row = find_row_containing(&buffer, "Alpha");
    let col = find_col_containing(&buffer, row, "A");
    let cell = buffer.cell((col, row)).unwrap();
    assert_eq!(cell.bg, Color::Blue);
}

#[test]
fn test_selected_project_row_uses_the_configured_background_highlight() {
    let mut app = seed_two_boards();

    let default_buffer = render_buffer(&mut app);
    let default_row = find_row_containing(&default_buffer, "Alpha");
    let default_col = find_col_containing(&default_buffer, default_row, "A");
    let default_fg = default_buffer.cell((default_col, default_row)).unwrap().fg;

    app.app_config.background_highlight = Some("#2E3440".to_string());

    let buffer = render_buffer(&mut app);
    let row = find_row_containing(&buffer, "Alpha");
    let col = find_col_containing(&buffer, row, "A");
    let cell = buffer.cell((col, row)).unwrap();

    assert_eq!(cell.bg, Color::Rgb(0x2E, 0x34, 0x40));
    assert_eq!(cell.fg, default_fg);
}

#[test]
fn test_unselected_project_row_has_no_highlight_background() {
    let mut app = seed_two_boards();
    app.app_config.background_highlight = Some("#2E3440".to_string());

    let buffer = render_buffer(&mut app);
    let row = find_row_containing(&buffer, "Beta");
    let col = find_col_containing(&buffer, row, "B");
    let bg = buffer.cell((col, row)).unwrap().bg;
    assert_ne!(bg, Color::Blue);
    assert_ne!(bg, Color::Rgb(0x2E, 0x34, 0x40));
}

#[test]
fn test_sort_picker_selected_row_uses_the_configured_background_highlight() {
    use kanban_view::selection_dialog::SORT_FIELD_POPUP_ORDER;

    let mut app = seed_two_boards();
    app.app_config.background_highlight = Some("#2E3440".to_string());

    let board = app.ctx.list_boards().unwrap()[0].clone();
    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Cards;

    app.handle_order_cards_key();
    app.filter.sort_field_selection.set(Some(0));

    let buffer = render_buffer(&mut app);
    let label = SORT_FIELD_POPUP_ORDER[0].1;
    let row = find_row_containing(&buffer, label);
    let col = find_col_containing(&buffer, row, &label[0..1]);
    let cell = buffer.cell((col, row)).unwrap();
    assert_eq!(cell.bg, Color::Rgb(0x2E, 0x34, 0x40));
}

fn headless_terminal() -> Terminal<CrosstermBackend<io::Stdout>> {
    Terminal::with_options(
        CrosstermBackend::new(io::stdout()),
        TerminalOptions {
            viewport: Viewport::Fixed(ratatui::layout::Rect::new(0, 0, 80, 40)),
        },
    )
    .unwrap()
}

#[tokio::test]
async fn test_help_popup_selected_row_uses_the_configured_background_highlight() {
    let mut app = seed_two_boards();
    app.app_config.background_highlight = Some("#2E3440".to_string());
    app.focus.active = Focus::Boards;

    let mut terminal = headless_terminal();
    app.handle_key_event(
        KeyEvent::from(KeyCode::Char('?')),
        &mut terminal,
        &EventHandler::new(),
    );

    let buffer = render_buffer(&mut app);
    let title_row = find_row_containing(&buffer, "Help - Keybindings");
    let popup = kanban_tui::components::centered_rect(80, 80, buffer.area);

    let mut found = false;
    for y in (title_row + 1)..popup.bottom().saturating_sub(1) {
        for x in (popup.left() + 1)..popup.right().saturating_sub(1) {
            if let Some(cell) = buffer.cell((x, y)) {
                if cell.bg == Color::Rgb(0x2E, 0x34, 0x40) {
                    found = true;
                    break;
                }
            }
        }
        if found {
            break;
        }
    }
    assert!(
        found,
        "expected the first selected binding row inside the help popup to use the configured background highlight"
    );
}
