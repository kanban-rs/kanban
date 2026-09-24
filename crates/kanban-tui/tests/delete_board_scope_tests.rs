use crossterm::event::{KeyCode, KeyEvent};
use kanban_domain::{CreateCardOptions, KanbanOperations};
use kanban_tui::app::focus::Focus;
use kanban_tui::app::{AppMode, DialogMode};
use kanban_tui::events::EventHandler;
use kanban_tui::App;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::{Terminal, TerminalOptions, Viewport};
use std::io;

fn headless_terminal() -> Terminal<CrosstermBackend<io::Stdout>> {
    Terminal::with_options(
        CrosstermBackend::new(io::stdout()),
        TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, 80, 40)),
        },
    )
    .unwrap()
}

fn render_to_string(app: &mut App, width: u16, height: u16) -> String {
    use ratatui::backend::TestBackend;

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render(app, frame);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let mut result = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            result.push_str(buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
        result.push('\n');
    }
    result
}

#[tokio::test]
async fn test_pressing_delete_after_navigating_off_the_active_board_opens_the_confirm() {
    let mut app = App::test_default();

    let board1 = app.ctx.create_board("Board 1".to_string(), None).unwrap();
    let col1 = app
        .ctx
        .create_column(board1.id, "Todo".to_string(), None)
        .unwrap();
    app.ctx
        .create_card(
            board1.id,
            col1.id,
            "Card 1".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx
        .create_sprint(board1.id, None, Some("Sprint 1".to_string()))
        .unwrap();

    let board2 = app.ctx.create_board("Board 2".to_string(), None).unwrap();
    let col2 = app
        .ctx
        .create_column(board2.id, "Todo".to_string(), None)
        .unwrap();
    app.ctx
        .create_card(
            board2.id,
            col2.id,
            "Card 2".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx
        .create_card(
            board2.id,
            col2.id,
            "Card 3".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx
        .create_sprint(board2.id, None, Some("Sprint 2".to_string()))
        .unwrap();
    app.ctx
        .create_sprint(board2.id, None, Some("Sprint 3".to_string()))
        .unwrap();

    app.load_initial_state().await;

    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    app.focus.active = Focus::Boards;
    app.board_list.inner_mut().set_selected_index(Some(0));

    app.handle_key_event(KeyEvent::from(KeyCode::Enter), &mut terminal, &events);
    assert_eq!(app.selection.active_board_id, Some(board1.id));

    app.handle_key_event(KeyEvent::from(KeyCode::Char('1')), &mut terminal, &events);
    assert_eq!(
        app.focus.active,
        Focus::Boards,
        "focus is back on the projects panel"
    );
    assert_eq!(
        app.selection.active_board_id,
        Some(board1.id),
        "board 1 stays active across the focus switch"
    );

    app.handle_key_event(KeyEvent::from(KeyCode::Char('j')), &mut terminal, &events);
    assert_eq!(app.board_list.get_selected_board_id(), Some(board2.id));
    assert!(
        !app.model.board_columns_state(board2.id).is_loaded(),
        "precondition: board 2's subtree is not yet loaded"
    );

    app.handle_key_event(KeyEvent::from(KeyCode::Char('d')), &mut terminal, &events);

    assert_eq!(app.mode, AppMode::Dialog(DialogMode::DeleteBoardConfirm));
    assert!(app.ui_state.banner.is_none());
    assert_eq!(app.selection.active_board_id, Some(board1.id));

    let output = render_to_string(&mut app, 140, 30);
    assert!(
        output.contains("2 task(s)"),
        "expected board 2's task count, got:\n{}",
        output
    );
    assert!(
        output.contains("2 sprint(s)"),
        "expected board 2's sprint count, got:\n{}",
        output
    );
}
