mod helpers;

use kanban_domain::{
    field_update::FieldUpdate, CreateCardOptions, KanbanOperations, Snapshot, SprintUpdate,
};
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::App;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use ratatui::Terminal;

const TEST_BACKEND_WIDTH: u16 = 120;
const TEST_BACKEND_HEIGHT: u16 = 30;

fn configured_rgb() -> Color {
    Color::Rgb(0x2E, 0x34, 0x40)
}

fn render_app(app: &mut App) -> Buffer {
    let backend = TestBackend::new(TEST_BACKEND_WIDTH, TEST_BACKEND_HEIGHT);
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

fn setup_app_with_sprints() -> App {
    let mut app = App::test_default();
    let board = app.ctx.create_board("B".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".into(), None)
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Task".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    app.ctx.create_sprint(board.id, None, None).unwrap();

    let active = app.ctx.create_sprint(board.id, None, None).unwrap();
    app.ctx.activate_sprint(active.id, Some(7)).unwrap();
    let past = chrono::Utc::now() - chrono::Duration::days(1);
    app.ctx
        .update_sprint(
            active.id,
            SprintUpdate {
                end_date: FieldUpdate::Set(past),
                ..Default::default()
            },
        )
        .unwrap();

    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.selection.active_card_id = Some(card.id);
    app.reload_model();
    app.prepare_frame();

    app
}

fn open_assign_dialog(app: &mut App) {
    let current_sprint_id = app
        .selection
        .active_card_id
        .and_then(|id| app.model.card_by_id_state(id).loaded().copied())
        .and_then(|c| c.sprint_id);
    let board = app
        .model
        .boards_state()
        .loaded_or_empty()
        .first()
        .cloned()
        .expect("test app has at least one board");
    let sprints = app
        .model
        .board_sprints_state(board.id)
        .loaded()
        .copied()
        .unwrap_or(&[])
        .to_vec();
    app.dialog_input
        .assign_sprint_picker
        .reset_for_card_assignment(current_sprint_id, &sprints, &board, chrono::Utc::now());
    app.push_mode(AppMode::Dialog(DialogMode::AssignCardToSprint));
}

#[test]
fn test_assign_sprint_dialog_cursor_row_uses_the_configured_background_highlight() {
    let mut app = setup_app_with_sprints();
    app.app_config.background_highlight = Some("#2E3440".to_string());
    open_assign_dialog(&mut app);

    let buffer = render_app(&mut app);
    let (x, y) = find_cell_position(&buffer, "(None)");
    let cell = buffer.cell((x, y)).unwrap();
    assert_eq!(cell.style().bg, Some(configured_rgb()));
    assert_eq!(cell.style().fg, Some(Color::White));
}

#[test]
fn test_manage_parents_popup_selected_row_uses_the_configured_background_highlight() {
    let mut app = App::test_default();
    app.app_config.background_highlight = Some("#2E3440".to_string());

    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "TODO".into(), None)
        .unwrap();
    let parent = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Parent".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let child = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Child".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: app.ctx.data_store().list_boards().unwrap(),
        columns: app.ctx.data_store().list_all_columns().unwrap(),
        cards: app.ctx.data_store().list_all_cards().unwrap(),
        archived_cards: app.ctx.data_store().list_archived_cards().unwrap(),
        sprints: app.ctx.data_store().list_all_sprints().unwrap(),
        graph: app.ctx.data_store().get_graph().unwrap(),
        prefixes: Vec::new(),
    };
    app.load_snapshot(snapshot);
    app.selection.active_card_id = Some(child.id);

    app.push_mode(AppMode::Dialog(DialogMode::ManageParents));
    app.relationship.card_ids = vec![parent.id];
    app.relationship.selection.set(Some(0));

    let buffer = render_app(&mut app);
    let (x, y) = find_cell_position(&buffer, "] Parent");
    let cell = buffer.cell((x, y)).unwrap();
    assert_eq!(cell.style().bg, Some(configured_rgb()));
    assert_eq!(cell.style().fg, Some(Color::White));
}
