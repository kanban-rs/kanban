//! Each test seeds one live + one archived card sharing the same
//! sprint/column and asserts the displayed count/list reflects only the
//! live card.

use kanban_domain::KanbanOperations;
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::app::BoardFocus;
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

fn activate_board(app: &mut App, board_id: uuid::Uuid) {
    app.reload_model();
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = Some(board_id);
}

#[test]
fn test_sprint_detail_card_count_excludes_archived_cards() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let sprint = app.ctx.create_sprint(board.id, None, None).unwrap();

    let live = app
        .ctx
        .create_card(board.id, col.id, "Live".to_string(), Default::default())
        .unwrap();
    app.ctx.assign_card_to_sprint(live.id, sprint.id).unwrap();

    let archived = app
        .ctx
        .create_card(board.id, col.id, "Archived".to_string(), Default::default())
        .unwrap();
    app.ctx
        .assign_card_to_sprint(archived.id, sprint.id)
        .unwrap();
    app.ctx.archive_card(archived.id).unwrap();

    activate_board(&mut app, board.id);
    app.selection.active_sprint_index = Some(0);
    app.push_mode(AppMode::SprintDetail);
    app.reload_model();
    app.prepare_frame();

    let output = render_to_string(&mut app, 100, 30);

    assert!(
        output.contains("Cards Assigned: 1"),
        "Sprint Detail's Cards Assigned count must exclude the archived card, got:\n{}",
        output
    );
    assert!(
        !output.contains("Cards Assigned: 2"),
        "count must not include the archived card, got:\n{}",
        output
    );
}

#[test]
fn test_board_detail_sprint_card_count_excludes_archived_cards() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let sprint = app
        .ctx
        .create_sprint(board.id, None, Some("TargetSprint".to_string()))
        .unwrap();

    let live = app
        .ctx
        .create_card(board.id, col.id, "Live".to_string(), Default::default())
        .unwrap();
    app.ctx.assign_card_to_sprint(live.id, sprint.id).unwrap();

    let archived = app
        .ctx
        .create_card(board.id, col.id, "Archived".to_string(), Default::default())
        .unwrap();
    app.ctx
        .assign_card_to_sprint(archived.id, sprint.id)
        .unwrap();
    app.ctx.archive_card(archived.id).unwrap();

    activate_board(&mut app, board.id);
    app.push_mode(AppMode::BoardDetail);
    app.focus.board_focus = BoardFocus::Sprints;
    app.reload_model();
    app.prepare_frame();

    let output = render_to_string(&mut app, 100, 30);

    assert!(
        output.contains("TargetSprint (1)"),
        "Board Detail's per-sprint card count must exclude the archived card, got:\n{}",
        output
    );
    assert!(
        !output.contains("TargetSprint (2)"),
        "count must not include the archived card, got:\n{}",
        output
    );
}

#[test]
fn test_board_detail_column_card_count_excludes_archived_cards() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let col = app
        .ctx
        .create_column(board.id, "TargetColumn".to_string(), None)
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

    activate_board(&mut app, board.id);
    app.push_mode(AppMode::BoardDetail);
    app.focus.board_focus = BoardFocus::Columns;
    app.reload_model();
    app.prepare_frame();

    let output = render_to_string(&mut app, 100, 30);

    assert!(
        output.contains("TargetColumn (1)"),
        "Board Detail's per-column card count must exclude the archived card, got:\n{}",
        output
    );
    assert!(
        !output.contains("TargetColumn (2)"),
        "count must not include the archived card, got:\n{}",
        output
    );
}

#[test]
fn test_carry_over_popup_card_count_excludes_archived_cards() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let source_sprint = app.ctx.create_sprint(board.id, None, None).unwrap();

    let live = app
        .ctx
        .create_card(board.id, col.id, "Live".to_string(), Default::default())
        .unwrap();
    app.ctx
        .assign_card_to_sprint(live.id, source_sprint.id)
        .unwrap();

    let archived = app
        .ctx
        .create_card(board.id, col.id, "Archived".to_string(), Default::default())
        .unwrap();
    app.ctx
        .assign_card_to_sprint(archived.id, source_sprint.id)
        .unwrap();
    app.ctx.archive_card(archived.id).unwrap();

    activate_board(&mut app, board.id);
    app.dialog_input.carry_over_source_sprint_id = Some(source_sprint.id);
    app.push_mode(AppMode::Dialog(DialogMode::CarryOverSprint));
    app.reload_model();
    app.prepare_frame();

    let output = render_to_string(&mut app, 100, 30);

    assert!(
        output.contains("Carry Over to Sprint (1 cards)"),
        "Carry-over popup's card count must exclude the archived card, got:\n{}",
        output
    );
    assert!(
        !output.contains("Carry Over to Sprint (2 cards)"),
        "count must not include the archived card, got:\n{}",
        output
    );
}

#[test]
fn test_completed_sprint_uncompleted_panel_excludes_archived_cards() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let sprint = app.ctx.create_sprint(board.id, None, None).unwrap();
    app.ctx.activate_sprint(sprint.id, None).unwrap();

    let live = app
        .ctx
        .create_card(board.id, col.id, "Live".to_string(), Default::default())
        .unwrap();
    app.ctx.assign_card_to_sprint(live.id, sprint.id).unwrap();

    let archived = app
        .ctx
        .create_card(board.id, col.id, "Archived".to_string(), Default::default())
        .unwrap();
    app.ctx
        .assign_card_to_sprint(archived.id, sprint.id)
        .unwrap();
    app.ctx.archive_card(archived.id).unwrap();

    app.ctx.complete_sprint(sprint.id).unwrap();

    activate_board(&mut app, board.id);
    app.populate_sprint_task_lists(sprint.id);

    assert_eq!(
        app.sprint_view.uncompleted_cards.cards,
        vec![live.id],
        "completed sprint's Uncompleted panel must exclude the archived card"
    );
}

#[test]
fn test_carry_over_auto_open_gate_excludes_archived_cards() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let sprint = app.ctx.create_sprint(board.id, None, None).unwrap();
    app.ctx.activate_sprint(sprint.id, None).unwrap();
    // A second Planning sprint must exist for carry-over to be offered at all.
    app.ctx.create_sprint(board.id, None, None).unwrap();

    let archived = app
        .ctx
        .create_card(board.id, col.id, "Archived".to_string(), Default::default())
        .unwrap();
    app.ctx
        .assign_card_to_sprint(archived.id, sprint.id)
        .unwrap();
    app.ctx.archive_card(archived.id).unwrap();

    activate_board(&mut app, board.id);
    app.selection.active_sprint_index = Some(
        app.model
            .sprints()
            .iter()
            .position(|s| s.id == sprint.id)
            .unwrap(),
    );

    app.handle_complete_sprint_key();
    app.reload_model();
    app.prepare_frame();

    assert_ne!(
        app.mode,
        AppMode::Dialog(DialogMode::CarryOverSprint),
        "carry-over popup must not auto-open when the only uncompleted card is archived"
    );
}
