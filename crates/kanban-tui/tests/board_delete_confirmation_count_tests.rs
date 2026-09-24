//! The board delete/archive confirmation dialog's "task(s)" count currently
//! includes archived cards sitting in a to-be-deleted column, double-counting
//! them against the separate "archived task(s)" figure.

mod helpers;

use helpers::CountingBackend;
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

    app.reload_model();
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Boards;

    app.handle_delete_board_key();
    app.reload_model();
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

#[test]
fn test_the_delete_board_confirmation_reports_the_archived_count_after_a_lazy_populate() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let _live = app
        .ctx
        .create_card(board.id, col.id, "Live".to_string(), Default::default())
        .unwrap();
    let archived = app
        .ctx
        .create_card(board.id, col.id, "Archived".to_string(), Default::default())
        .unwrap();
    app.ctx.archive_card(archived.id).unwrap();

    app.populate(kanban_tui::app::ViewScope {
        board_list: true,
        board: Some(board.id),
        board_columns: true,
        board_cards: true,
        board_sprints: true,
        ..Default::default()
    });
    app.prepare_frame();

    assert!(
        !app.model.archived_card_markers_absorbed(),
        "a lazy populate must not absorb the archived marker tier"
    );

    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Boards;

    app.handle_delete_board_key();

    assert!(
        app.ui_state.banner.is_none(),
        "the delete-board confirmation must open after absorbing the archived tier, got banner:\n{:?}",
        app.ui_state.banner
    );

    let output = render_to_string(&mut app, 100, 30);

    assert!(
        output.contains("1 archived task(s)"),
        "the archived task count must reflect the one archived card, got:\n{}",
        output
    );
    assert!(
        output.contains("1 task(s)"),
        "the live task count must be reported, got:\n{}",
        output
    );
}

/// `board_delete_counts` reads the by-board scoped archived-card tier, not
/// the flat global one. A failure on the flat `list_archived_cards` read
/// must not blank the archived count in the confirmation dialog.
#[test]
fn test_board_delete_confirmation_reports_counts_when_the_global_archived_tier_fails() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();

    let _live = app
        .ctx
        .create_card(board.id, col.id, "Live".to_string(), Default::default())
        .unwrap();
    let archived = app
        .ctx
        .create_card(board.id, col.id, "Archived".to_string(), Default::default())
        .unwrap();
    app.ctx.archive_card(archived.id).unwrap();

    app.selection.active_board_id = Some(board.id);
    app.reload_model();
    app.prepare_frame();

    let failing = CountingBackend::wrap_failing(app.ctx.backend(), "list_archived_cards");
    app.ctx.replace_backend(failing);
    app.reload_model();
    app.prepare_frame();

    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Boards;

    app.handle_delete_board_key();
    app.reload_model();
    app.prepare_frame();

    let output = render_to_string(&mut app, 100, 30);

    assert!(
        output.contains("1 task(s)"),
        "the live task count must still be reported with the global archived \
         tier failing, got:\n{}",
        output
    );
    assert!(
        output.contains("1 archived task(s)"),
        "the archived task count must come from the by-board tier, not the \
         failing global one, got:\n{}",
        output
    );
}
