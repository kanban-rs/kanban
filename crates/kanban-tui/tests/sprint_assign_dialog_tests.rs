mod helpers;

use chrono::{Duration, Utc};
use crossterm::event::KeyCode;
use kanban_domain::{field_update::FieldUpdate, CreateCardOptions, KanbanOperations, SprintUpdate};
use kanban_tui::{
    app::mode::{AppMode, DialogMode},
    components::{SelectionDialog, SprintAssignDialog},
    App,
};
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::Terminal;
use uuid::Uuid;

const TEST_BACKEND_WIDTH: u16 = 120;
const TEST_BACKEND_HEIGHT: u16 = 30;

struct DialogFixture {
    app: App,
    card_id: Uuid,
    completed_id: Uuid,
    ended_id: Uuid,
}

fn setup_app_with_sprints() -> DialogFixture {
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
    let past = Utc::now() - Duration::days(1);
    app.ctx
        .update_sprint(
            active.id,
            SprintUpdate {
                end_date: FieldUpdate::Set(past),
                ..Default::default()
            },
        )
        .unwrap();

    let to_complete = app.ctx.create_sprint(board.id, None, None).unwrap();
    app.ctx.activate_sprint(to_complete.id, Some(7)).unwrap();
    app.ctx.complete_sprint(to_complete.id).unwrap();

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

    DialogFixture {
        app,
        card_id: card.id,
        completed_id: to_complete.id,
        ended_id: active.id,
    }
}

fn open_assign_dialog(app: &mut App) {
    let current_sprint_id = app
        .selection
        .active_card_id
        .and_then(|id| app.model.card_by_id(id))
        .and_then(|c| c.sprint_id);
    let sprints = app.model.sprints().to_vec();
    let board = app
        .model
        .boards()
        .first()
        .cloned()
        .expect("test app has at least one board");
    app.dialog_input
        .assign_sprint_picker
        .reset_for_card_assignment(current_sprint_id, &sprints, &board, Utc::now());
    app.push_mode(AppMode::Dialog(DialogMode::AssignCardToSprint));
}

fn render_dialog_to_string(app: &App) -> String {
    let backend = TestBackend::new(TEST_BACKEND_WIDTH, TEST_BACKEND_HEIGHT);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let dialog = SprintAssignDialog;
            dialog.render(app, frame);
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

fn render_dialog_with_colors(app: &App) -> Vec<(String, Option<Color>)> {
    let backend = TestBackend::new(TEST_BACKEND_WIDTH, TEST_BACKEND_HEIGHT);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let dialog = SprintAssignDialog;
            dialog.render(app, frame);
        })
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    let mut result = Vec::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y)).unwrap();
            result.push((cell.symbol().to_string(), cell.style().fg));
        }
    }
    result
}

fn line_color(grid: &[(String, Option<Color>)], substring: &str) -> Option<Color> {
    let width = TEST_BACKEND_WIDTH as usize;
    let height = grid.len() / width;
    for y in 0..height {
        let line: String = (0..width)
            .map(|x| grid[y * width + x].0.clone())
            .collect::<Vec<_>>()
            .join("");
        if let Some(start) = line.find(substring) {
            // Return the fg color of the first cell of the substring.
            return grid[y * width + start].1;
        }
    }
    None
}

#[test]
fn test_dialog_renders_active_planned_header_when_active_sprints_exist() {
    let fx = setup_app_with_sprints();
    let mut app = fx.app;
    open_assign_dialog(&mut app);
    let output = render_dialog_to_string(&app);
    assert!(
        output.contains("Active / Planned"),
        "expected Active / Planned header in dialog output:\n{}",
        output
    );
}

#[test]
fn test_dialog_renders_completed_ended_header_when_either_completed_or_ended_exist() {
    let fx = setup_app_with_sprints();
    let mut app = fx.app;
    open_assign_dialog(&mut app);
    let output = render_dialog_to_string(&app);
    assert!(
        output.contains("Completed / Ended"),
        "expected Completed / Ended header in dialog output:\n{}",
        output
    );
}

#[test]
fn test_dialog_renders_completed_in_green_and_ended_in_red() {
    let fx = setup_app_with_sprints();
    let mut app = fx.app;
    open_assign_dialog(&mut app);

    // Find the completed and ended sprint by their formatted names.
    let completed = app
        .model
        .sprints()
        .iter()
        .find(|s| s.id == fx.completed_id)
        .cloned()
        .unwrap();
    let ended = app
        .model
        .sprints()
        .iter()
        .find(|s| s.id == fx.ended_id)
        .cloned()
        .unwrap();
    let board = app.model.boards()[0].clone();
    let completed_name = completed.formatted_name(&board, "sprint");
    let ended_name = ended.formatted_name(&board, "sprint");

    let grid = render_dialog_with_colors(&app);
    let completed_color = line_color(&grid, &completed_name);
    let ended_color = line_color(&grid, &ended_name);

    assert_eq!(
        completed_color,
        Some(Color::Green),
        "completed sprint should be green; got {:?} for {:?}",
        completed_color,
        completed_name
    );
    assert_eq!(
        ended_color,
        Some(Color::Red),
        "ended sprint should be red; got {:?} for {:?}",
        ended_color,
        ended_name
    );
}

#[test]
fn test_down_arrow_skips_headers() {
    let fx = setup_app_with_sprints();
    let mut app = fx.app;
    open_assign_dialog(&mut app);
    // The fixture's card has no current sprint, so the picker opens
    // with the cursor on the (None) row.
    assert_eq!(
        app.dialog_input.assign_sprint_picker.cursor_sprint_id(),
        None
    );

    // One Down lands on a real sprint row — the Active/Planned section
    // header gets skipped by next_selectable.
    app.handle_assign_card_to_sprint_popup(KeyCode::Char('j'));
    assert!(
        app.dialog_input
            .assign_sprint_picker
            .cursor_sprint_id()
            .is_some(),
        "Down should land the cursor on a sprint row, not a header"
    );
}

fn walk_cursor_to_sprint(app: &mut App, target: Uuid, max_steps: usize) {
    for _ in 0..max_steps {
        if app.dialog_input.assign_sprint_picker.cursor_sprint_id() == Some(target) {
            return;
        }
        app.handle_assign_card_to_sprint_popup(KeyCode::Char('j'));
    }
    panic!("could not navigate cursor to target sprint within {max_steps} steps");
}

#[test]
fn test_assigning_to_completed_sprint_succeeds() {
    let fx = setup_app_with_sprints();
    let mut app = fx.app;
    let card_id = fx.card_id;
    let target_sprint = fx.completed_id;

    open_assign_dialog(&mut app);
    walk_cursor_to_sprint(&mut app, target_sprint, 20);
    // Space commits the cursor's row as the [x] selection, Enter applies.
    app.handle_assign_card_to_sprint_popup(KeyCode::Char(' '));
    app.handle_assign_card_to_sprint_popup(KeyCode::Enter);

    let card = app.ctx.get_card(card_id).unwrap().unwrap();
    assert_eq!(
        card.sprint_id,
        Some(target_sprint),
        "card should be assigned to the completed sprint"
    );
}

#[test]
fn test_assigning_to_ended_sprint_succeeds() {
    let fx = setup_app_with_sprints();
    let mut app = fx.app;
    let card_id = fx.card_id;
    let target_sprint = fx.ended_id;

    open_assign_dialog(&mut app);
    walk_cursor_to_sprint(&mut app, target_sprint, 20);
    app.handle_assign_card_to_sprint_popup(KeyCode::Char(' '));
    app.handle_assign_card_to_sprint_popup(KeyCode::Enter);

    let card = app.ctx.get_card(card_id).unwrap().unwrap();
    assert_eq!(
        card.sprint_id,
        Some(target_sprint),
        "card should be assigned to the ended sprint"
    );
}

#[test]
fn test_bulk_assign_bare_enter_is_a_no_op_and_does_not_mass_unassign() {
    // Regression guard for KAN-556 review: opening the bulk-assign
    // dialog and pressing Enter without first touching the picker must
    // leave every selected card's sprint_id untouched. Previously the
    // dialog opened with Selection::NoSprint, so a bare Enter
    // unassigned the entire selection.
    let fx = setup_app_with_sprints();
    let mut app = fx.app;
    let card_id = fx.card_id;

    // Pre-assign the card to a sprint so we can detect the regression
    // (unassign-on-open would clear sprint_id).
    let sprints = app.model.sprints().to_vec();
    let board = app.model.boards().first().cloned().unwrap();
    let some_sprint = sprints
        .iter()
        .find(|s| s.board_id == board.id)
        .map(|s| s.id)
        .unwrap();
    app.ctx.assign_card_to_sprint(card_id, some_sprint).unwrap();
    app.reload_model();
    app.prepare_frame();

    // Open bulk dialog with no interaction, then press Enter immediately.
    app.multi_select.selected_cards.insert(card_id);
    app.dialog_input
        .assign_sprint_picker
        .reset_for_bulk_card_assignment(&sprints, &board, Utc::now());
    app.push_mode(AppMode::Dialog(DialogMode::AssignMultipleCardsToSprint));
    app.handle_assign_multiple_cards_to_sprint_popup(KeyCode::Enter);

    let card = app.ctx.get_card(card_id).unwrap().unwrap();
    assert_eq!(
        card.sprint_id,
        Some(some_sprint),
        "bare Enter on bulk-assign must not clobber the pre-existing assignment"
    );
}

#[test]
fn test_bulk_assign_handler_supports_completed_sprint() {
    let fx = setup_app_with_sprints();
    let mut app = fx.app;
    let card_id = fx.card_id;
    let target_sprint = fx.completed_id;

    // Switch to bulk-assign flow with one selected card, with picker
    // primed (no single "current" sprint in bulk mode).
    app.multi_select.selected_cards.insert(card_id);
    let sprints = app.model.sprints().to_vec();
    let board = app.model.boards().first().cloned().unwrap();
    app.dialog_input
        .assign_sprint_picker
        .reset_for_bulk_card_assignment(&sprints, &board, Utc::now());
    app.push_mode(AppMode::Dialog(DialogMode::AssignMultipleCardsToSprint));

    let max_steps = 20;
    for _ in 0..max_steps {
        if app.dialog_input.assign_sprint_picker.cursor_sprint_id() == Some(target_sprint) {
            break;
        }
        app.handle_assign_multiple_cards_to_sprint_popup(KeyCode::Char('j'));
    }
    app.handle_assign_multiple_cards_to_sprint_popup(KeyCode::Char(' '));
    app.handle_assign_multiple_cards_to_sprint_popup(KeyCode::Enter);

    let card = app.ctx.get_card(card_id).unwrap().unwrap();
    assert_eq!(
        card.sprint_id,
        Some(target_sprint),
        "bulk-assigned card should land on the completed sprint"
    );
}

#[test]
fn test_current_sprint_indicator_does_not_apply_color_override_in_completed_ended_section() {
    let fx = setup_app_with_sprints();
    let mut app = fx.app;
    let card_id = fx.card_id;
    let completed_id = fx.completed_id;

    // Pre-assign card to the completed sprint (simulates the retrospective
    // scenario where the user previously assigned to a Completed sprint).
    app.ctx
        .assign_card_to_sprint(card_id, completed_id)
        .unwrap();
    app.reload_model();
    app.prepare_frame();

    open_assign_dialog(&mut app);
    // The dialog opens with cursor + checkbox on the current sprint —
    // which is the completed one here. Move the cursor off that row
    // (onto the (None) row at the top) so the focus-blue does not
    // dominate the row's color, leaving the status colour test
    // meaningful.
    while app
        .dialog_input
        .assign_sprint_picker
        .cursor_sprint_id()
        .is_some()
    {
        app.handle_assign_card_to_sprint_popup(KeyCode::Char('k'));
    }

    let board = app.model.boards()[0].clone();
    let completed = app
        .model
        .sprints()
        .iter()
        .find(|s| s.id == completed_id)
        .cloned()
        .unwrap();
    let completed_name = completed.formatted_name(&board, "sprint");

    let grid = render_dialog_with_colors(&app);

    // Cursor is off the completed row, so its row keeps the Completed
    // status colour (green) instead of the cursor-focus blue.
    let color = line_color(&grid, &completed_name);
    assert_eq!(
        color,
        Some(Color::Green),
        "completed-current sprint must keep green status colour, got {:?}",
        color
    );

    // It should also carry a " (current)" suffix for identification.
    let output = render_dialog_to_string(&app);
    assert!(
        output.contains(&format!("{} (current)", completed_name)),
        "expected ' (current)' suffix on the completed-current entry; output:\n{}",
        output
    );
}

#[test]
fn test_dialog_scrolls_to_keep_selected_sprint_visible_when_list_overflows() {
    // 30 planning sprints overflows the dialog's visible area at the
    // standard 120x30 test backend size.
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
    for _ in 0..30 {
        app.ctx.create_sprint(board.id, None, None).unwrap();
    }
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

    open_assign_dialog(&mut app);

    // Sprints are sorted by sprint_number desc, so the oldest sprint sits at
    // the bottom of the active section — guaranteed off-screen without scroll.
    let oldest_id = app
        .model
        .sprints()
        .iter()
        .min_by_key(|s| s.sprint_number)
        .map(|s| s.id)
        .unwrap();

    walk_cursor_to_sprint(&mut app, oldest_id, 50);

    let board_after = app.model.boards()[0].clone();
    let oldest = app
        .model
        .sprints()
        .iter()
        .find(|s| s.id == oldest_id)
        .cloned()
        .unwrap();
    let oldest_name = oldest.formatted_name(&board_after, "sprint");

    let output = render_dialog_to_string(&app);
    assert!(
        output.contains(&oldest_name),
        "selected sprint at end of long list must remain visible after scroll; output:\n{}",
        output
    );
}

#[test]
fn test_sticky_header_appears_at_top_when_scrolled_past_active_planned_header() {
    // 30 planning sprints — selected at the last entry forces the
    // Active / Planned header (at entry index 1) to scroll off the top.
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
    for _ in 0..30 {
        app.ctx.create_sprint(board.id, None, None).unwrap();
    }
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

    open_assign_dialog(&mut app);

    // Navigate to the last selectable entry.
    for _ in 0..50 {
        app.handle_assign_card_to_sprint_popup(KeyCode::Char('j'));
    }

    let output = render_dialog_to_string(&app);
    assert!(
        output.contains("Active / Planned"),
        "Active / Planned header must be pinned at the top of the list area when scrolled past; output:\n{}",
        output
    );
}

#[test]
fn test_sticky_header_does_not_duplicate_when_list_fits_without_scrolling() {
    // Short list — header sits at its natural position; no overlay should
    // render so the label appears exactly once in the output.
    let fx = setup_app_with_sprints();
    let mut app = fx.app;
    open_assign_dialog(&mut app);

    let output = render_dialog_to_string(&app);
    let count = output.matches("Active / Planned").count();
    assert_eq!(
        count, 1,
        "without scroll, Active / Planned should appear exactly once (natural position); got {} in:\n{}",
        count, output
    );
}

#[test]
fn test_sticky_header_switches_to_completed_ended_when_selecting_in_lower_section() {
    // 30 planning + 30 completed sprints. With the selection on the last
    // completed entry, both section headers have scrolled off — the overlay
    // should pin the *enclosing* section's header (Completed / Ended).
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
    for _ in 0..30 {
        app.ctx.create_sprint(board.id, None, None).unwrap();
    }
    for _ in 0..30 {
        let s = app.ctx.create_sprint(board.id, None, None).unwrap();
        app.ctx.activate_sprint(s.id, Some(7)).unwrap();
        app.ctx.complete_sprint(s.id).unwrap();
    }
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

    open_assign_dialog(&mut app);

    // Walk all the way to the last entry — guaranteed to be a completed sprint
    // with the lower section's header scrolled off.
    for _ in 0..200 {
        app.handle_assign_card_to_sprint_popup(KeyCode::Char('j'));
    }

    let output = render_dialog_to_string(&app);
    assert!(
        output.contains("Completed / Ended"),
        "Completed / Ended must be pinned at the top when selection is in the lower section; output:\n{}",
        output
    );
    // The upper section's header is also scrolled off but should NOT appear —
    // the overlay shows the *enclosing* section header for the selection.
    assert!(
        !output.contains("Active / Planned"),
        "Active / Planned must not appear when selection is past it and the overlay belongs to the lower section; output:\n{}",
        output
    );
}
