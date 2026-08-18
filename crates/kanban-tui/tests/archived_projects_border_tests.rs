use kanban_domain::{CreateCardOptions, KanbanOperations};
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::App;
use ratatui::style::Color;

fn border_fg_at(app: &mut App, x: u16, y: u16) -> Option<Color> {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render(app, frame);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    buffer.cell((x, y)).map(|c| c.style().fg).unwrap()
}

fn seed_archived_boards_view(name: &str) -> App {
    let mut app = App::test_default();
    let board = app.ctx.create_board(name.to_string(), None).unwrap();
    app.ctx.archive_board(board.id).unwrap();
    app.mode = AppMode::ArchivedBoardsView;
    app.focus.active = Focus::Boards;
    app.reload_model();
    app.prepare_frame();
    app
}

#[test]
fn test_the_archived_projects_panel_uses_the_archived_border() {
    let mut app = seed_archived_boards_view("Archived");

    let fg = border_fg_at(&mut app, 0, 0);

    assert_eq!(
        fg,
        kanban_tui::theme::deleted_view_focused_border().fg,
        "the archived projects panel border must use the archived-view border style"
    );
}

#[test]
fn test_the_archived_projects_border_survives_a_dialog_overlay() {
    let mut app = seed_archived_boards_view("Archived");
    app.push_mode(AppMode::Dialog(DialogMode::DeletePermanentBoardConfirm));

    let fg = border_fg_at(&mut app, 0, 0);

    assert_eq!(
        fg,
        kanban_tui::theme::deleted_view_focused_border().fg,
        "the archived border must survive a dialog opened over the archived view"
    );
}

#[test]
fn test_the_live_projects_panel_border_is_unchanged() {
    let mut app = App::test_default();
    app.ctx.create_board("Live".to_string(), None).unwrap();
    app.mode = AppMode::Normal;
    app.focus.active = Focus::Boards;
    app.reload_model();
    app.prepare_frame();

    let fg = border_fg_at(&mut app, 0, 0);

    assert_eq!(
        fg,
        kanban_tui::theme::focused_border().fg,
        "the live, focused projects panel border must stay the normal focused colour"
    );
}

#[test]
fn test_the_unfocused_archived_projects_panel_is_not_tinted() {
    let mut app = seed_archived_boards_view("Archived");
    app.focus.active = Focus::Cards;

    let fg = border_fg_at(&mut app, 0, 0);

    assert_eq!(
        fg,
        kanban_tui::theme::unfocused_border().fg,
        "an unfocused archived projects panel must not be tinted"
    );
}

#[test]
fn test_the_archived_cards_panel_border_is_unaffected() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "ArchiveMe".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.archive_card(card.id).unwrap();

    app.selection.active_board_id = Some(board.id);
    app.mode = AppMode::ArchivedCardsView;
    app.focus.active = Focus::Cards;
    app.reload_model();
    app.prepare_frame();

    let fg = border_fg_at(&mut app, 36, 0);

    assert_eq!(
        fg,
        Some(Color::Yellow),
        "the archived cards panel border must remain tinted as before"
    );
}
