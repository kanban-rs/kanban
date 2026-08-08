//! The board delete/archive confirmation dialog's "task(s)" count currently
//! includes archived cards sitting in a to-be-deleted column, double-counting
//! them against the separate "archived task(s)" figure.

use kanban_domain::KanbanOperations;
use kanban_tui::app::focus::Focus;
use kanban_tui::App;

fn render_to_string(app: &mut App, width: u16, height: u16) -> String {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

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

#[test]
fn test_board_delete_confirmation_card_count_excludes_archived_cards() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();

    let live = app
        .ctx
        .create_card(board.id, col.id, "Live".to_string(), Default::default())
        .unwrap();
    let archived = app
        .ctx
        .create_card(board.id, col.id, "Archived".to_string(), Default::default())
        .unwrap();
    app.ctx.archive_card(archived.id).unwrap();
    let _ = live;

    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Boards;

    app.handle_delete_board_key();
    app.prepare_frame();

    let output = render_to_string(&mut app, 100, 30);

    assert!(
        output.contains("1 task(s)"),
        "the live task count must exclude the archived card, got:\n{}",
        output
    );
    assert!(
        !output.contains("2 task(s)"),
        "count must not double-count the archived card, got:\n{}",
        output
    );
    assert!(
        output.contains("1 archived task(s)"),
        "the archived task count must remain 1, got:\n{}",
        output
    );
}
