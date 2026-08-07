use chrono::{Duration, Utc};
use kanban_domain::{field_update::FieldUpdate, CreateCardOptions, KanbanOperations, SprintUpdate};
use kanban_tui::components::SprintPickerView;
use kanban_tui::App;
use kanban_view::sprint_assign_list::{build_entries, sprint_id_of};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::Terminal;
use uuid::Uuid;

const TEST_W: u16 = 80;
const TEST_H: u16 = 20;

fn make_app_with_board() -> (App, Uuid, Uuid) {
    let mut app = App::test_default();
    let board = app.ctx.create_board("B".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".into(), None)
        .unwrap();
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.prepare_frame();
    (app, board.id, column.id)
}

fn add_planning_sprint(app: &mut App, board_id: Uuid) -> Uuid {
    let id = app.ctx.create_sprint(board_id, None, None).unwrap().id;
    app.prepare_frame();
    id
}

fn add_active_sprint(app: &mut App, board_id: Uuid) -> Uuid {
    let sprint = app.ctx.create_sprint(board_id, None, None).unwrap();
    app.ctx.activate_sprint(sprint.id, Some(7)).unwrap();
    app.prepare_frame();
    sprint.id
}

fn add_ended_sprint(app: &mut App, board_id: Uuid) -> Uuid {
    let sprint = app.ctx.create_sprint(board_id, None, None).unwrap();
    app.ctx.activate_sprint(sprint.id, Some(7)).unwrap();
    let past = Utc::now() - Duration::days(1);
    app.ctx
        .update_sprint(
            sprint.id,
            SprintUpdate {
                end_date: FieldUpdate::Set(past),
                ..Default::default()
            },
        )
        .unwrap();
    app.prepare_frame();
    sprint.id
}

fn add_completed_sprint(app: &mut App, board_id: Uuid) -> Uuid {
    let sprint = app.ctx.create_sprint(board_id, None, None).unwrap();
    app.ctx.activate_sprint(sprint.id, Some(7)).unwrap();
    app.ctx.complete_sprint(sprint.id).unwrap();
    app.prepare_frame();
    sprint.id
}

fn add_card(app: &mut App, board_id: Uuid, column_id: Uuid) -> Uuid {
    let id = app
        .ctx
        .create_card(
            board_id,
            column_id,
            "Task".into(),
            CreateCardOptions::default(),
        )
        .unwrap()
        .id;
    app.prepare_frame();
    id
}

fn assign_card_sprint(app: &mut App, card_id: Uuid, sprint_id: Uuid) {
    app.ctx.assign_card_to_sprint(card_id, sprint_id).unwrap();
    app.prepare_frame();
}

fn render_picker_to_string(picker: &SprintPickerView<'_>, selected: Option<usize>) -> String {
    let backend = TestBackend::new(TEST_W, TEST_H);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let area = Rect::new(0, 0, TEST_W, TEST_H);
            picker.render(frame, area, selected);
        })
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            out.push_str(buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
        out.push('\n');
    }
    out
}

fn render_picker_with_colors(
    picker: &SprintPickerView<'_>,
    selected: Option<usize>,
) -> Vec<(String, Option<Color>)> {
    let backend = TestBackend::new(TEST_W, TEST_H);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let area = Rect::new(0, 0, TEST_W, TEST_H);
            picker.render(frame, area, selected);
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
    let width = TEST_W as usize;
    let height = grid.len() / width;
    for y in 0..height {
        let line: String = (0..width)
            .map(|x| grid[y * width + x].0.clone())
            .collect::<Vec<_>>()
            .join("");
        if let Some(start) = line.find(substring) {
            return grid[y * width + start].1;
        }
    }
    None
}

#[test]
fn test_for_card_assignment_initial_selection_is_zero_when_card_has_no_sprint() {
    let (mut app, board_id, _col) = make_app_with_board();
    add_active_sprint(&mut app, board_id);
    add_planning_sprint(&mut app, board_id);
    let now = Utc::now();
    let board = app
        .model
        .boards()
        .iter()
        .find(|b| b.id == board_id)
        .cloned()
        .unwrap();
    let picker = SprintPickerView::for_card_assignment(app.model.sprints(), &board, None, now);
    assert_eq!(
        picker.initial_selection(),
        Some(0),
        "with no current sprint, initial selection should fall on the (None) entry"
    );
}

#[test]
fn test_for_card_assignment_initial_selection_is_index_of_current_sprint() {
    let (mut app, board_id, _col) = make_app_with_board();
    let active = add_active_sprint(&mut app, board_id);
    add_planning_sprint(&mut app, board_id);
    let now = Utc::now();
    let entries = build_entries(app.model.sprints(), board_id, now);
    let expected_idx = entries
        .iter()
        .position(|e| sprint_id_of(e) == Some(active))
        .expect("active sprint must appear in entries");
    let board = app
        .model
        .boards()
        .iter()
        .find(|b| b.id == board_id)
        .cloned()
        .unwrap();
    let picker =
        SprintPickerView::for_card_assignment(app.model.sprints(), &board, Some(active), now);
    assert_eq!(picker.initial_selection(), Some(expected_idx));
}

#[test]
fn test_for_card_assignment_render_shows_current_suffix_for_card_sprint() {
    let (mut app, board_id, col_id) = make_app_with_board();
    let active = add_active_sprint(&mut app, board_id);
    let card_id = add_card(&mut app, board_id, col_id);
    assign_card_sprint(&mut app, card_id, active);
    let now = Utc::now();
    let board = app
        .model
        .boards()
        .iter()
        .find(|b| b.id == board_id)
        .cloned()
        .unwrap();
    let picker =
        SprintPickerView::for_card_assignment(app.model.sprints(), &board, Some(active), now);
    let out = render_picker_to_string(&picker, picker.initial_selection());
    assert!(
        out.contains("(current)"),
        "(current) suffix should appear next to the card's active sprint: {}",
        out
    );
}

#[test]
fn test_for_new_card_preselects_sole_active_non_ended_sprint() {
    let (mut app, board_id, _col) = make_app_with_board();
    let active = add_active_sprint(&mut app, board_id);
    add_planning_sprint(&mut app, board_id);
    let now = Utc::now();
    let board = app
        .model
        .boards()
        .iter()
        .find(|b| b.id == board_id)
        .cloned()
        .unwrap();
    let picker = SprintPickerView::for_new_card(app.model.sprints(), &board, now);
    let expected_idx = picker
        .index_of_sprint(Some(active))
        .expect("active sprint must appear in the new-card picker");
    assert_eq!(
        picker.initial_selection(),
        Some(expected_idx),
        "the sole Active non-ended sprint should be pre-selected"
    );
}

#[test]
fn test_for_new_card_preselects_none_when_no_active_sprints() {
    let (mut app, board_id, _col) = make_app_with_board();
    add_planning_sprint(&mut app, board_id);
    add_completed_sprint(&mut app, board_id);
    let now = Utc::now();
    let board = app
        .model
        .boards()
        .iter()
        .find(|b| b.id == board_id)
        .cloned()
        .unwrap();
    let picker = SprintPickerView::for_new_card(app.model.sprints(), &board, now);
    assert_eq!(
        picker.initial_selection(),
        Some(0),
        "no Active sprint means the (None) entry stays pre-selected"
    );
}

#[test]
fn test_for_new_card_preselects_none_when_multiple_active_sprints() {
    let (mut app, board_id, _col) = make_app_with_board();
    add_active_sprint(&mut app, board_id);
    add_active_sprint(&mut app, board_id);
    let now = Utc::now();
    let board = app
        .model
        .boards()
        .iter()
        .find(|b| b.id == board_id)
        .cloned()
        .unwrap();
    let picker = SprintPickerView::for_new_card(app.model.sprints(), &board, now);
    assert_eq!(
        picker.initial_selection(),
        Some(0),
        "ambiguous active set must not auto-preselect"
    );
}

#[test]
fn test_value_at_returns_none_uuid_for_none_row() {
    let (mut app, board_id, _col) = make_app_with_board();
    add_active_sprint(&mut app, board_id);
    let now = Utc::now();
    let board = app
        .model
        .boards()
        .iter()
        .find(|b| b.id == board_id)
        .cloned()
        .unwrap();
    let picker = SprintPickerView::for_card_assignment(app.model.sprints(), &board, None, now);
    assert_eq!(
        picker.value_at(0),
        Some(None),
        "index 0 is the (None) entry; outer Some, inner None"
    );
}

#[test]
fn test_value_at_returns_sprint_id_for_sprint_row() {
    let (mut app, board_id, _col) = make_app_with_board();
    let active = add_active_sprint(&mut app, board_id);
    let now = Utc::now();
    let entries = build_entries(app.model.sprints(), board_id, now);
    let idx = entries
        .iter()
        .position(|e| sprint_id_of(e) == Some(active))
        .expect("active sprint must appear");
    let board = app
        .model
        .boards()
        .iter()
        .find(|b| b.id == board_id)
        .cloned()
        .unwrap();
    let picker = SprintPickerView::for_card_assignment(app.model.sprints(), &board, None, now);
    assert_eq!(picker.value_at(idx), Some(Some(active)));
}

#[test]
fn test_index_of_sprint_returns_row_index_for_known_sprint() {
    let (mut app, board_id, _col) = make_app_with_board();
    let active = add_active_sprint(&mut app, board_id);
    let _planning = add_planning_sprint(&mut app, board_id);
    let now = Utc::now();
    let board = app
        .model
        .boards()
        .iter()
        .find(|b| b.id == board_id)
        .cloned()
        .unwrap();
    let entries = build_entries(app.model.sprints(), board_id, now);
    let expected = entries.iter().position(|e| sprint_id_of(e) == Some(active));
    let picker = SprintPickerView::for_card_assignment(app.model.sprints(), &board, None, now);
    assert_eq!(picker.index_of_sprint(Some(active)), expected);
}

#[test]
fn test_index_of_sprint_returns_none_entry_index_when_no_sprint_selected() {
    let (mut app, board_id, _col) = make_app_with_board();
    add_active_sprint(&mut app, board_id);
    let now = Utc::now();
    let board = app
        .model
        .boards()
        .iter()
        .find(|b| b.id == board_id)
        .cloned()
        .unwrap();
    let picker = SprintPickerView::for_card_assignment(app.model.sprints(), &board, None, now);
    assert_eq!(
        picker.index_of_sprint(None),
        Some(0),
        "the (None) entry is always at index 0"
    );
}

#[test]
fn test_index_of_sprint_returns_none_when_sprint_is_not_in_list() {
    let (mut app, board_id, _col) = make_app_with_board();
    add_active_sprint(&mut app, board_id);
    let now = Utc::now();
    let board = app
        .model
        .boards()
        .iter()
        .find(|b| b.id == board_id)
        .cloned()
        .unwrap();
    let picker = SprintPickerView::for_card_assignment(app.model.sprints(), &board, None, now);
    let unknown = uuid::Uuid::new_v4();
    assert_eq!(picker.index_of_sprint(Some(unknown)), None);
}

#[test]
fn test_value_at_indices_match_build_entries_order() {
    let (mut app, board_id, _col) = make_app_with_board();
    add_active_sprint(&mut app, board_id);
    add_planning_sprint(&mut app, board_id);
    add_completed_sprint(&mut app, board_id);
    add_ended_sprint(&mut app, board_id);
    let now = Utc::now();
    let entries = build_entries(app.model.sprints(), board_id, now);
    let board = app
        .model
        .boards()
        .iter()
        .find(|b| b.id == board_id)
        .cloned()
        .unwrap();
    let picker = SprintPickerView::for_card_assignment(app.model.sprints(), &board, None, now);
    assert_eq!(
        picker.len(),
        entries.len(),
        "picker.len() must match build_entries length"
    );
    for (idx, entry) in entries.iter().enumerate() {
        let expected = match entry {
            kanban_view::sprint_assign_list::SprintAssignEntry::Header(_) => None,
            kanban_view::sprint_assign_list::SprintAssignEntry::None => Some(None),
            _ => Some(sprint_id_of(entry)),
        };
        assert_eq!(
            picker.value_at(idx),
            expected,
            "picker.value_at({}) must agree with build_entries[{}]",
            idx,
            idx
        );
    }
}

#[test]
fn test_render_emits_active_planned_header_with_yellow_color() {
    let (mut app, board_id, _col) = make_app_with_board();
    add_active_sprint(&mut app, board_id);
    let now = Utc::now();
    let board = app
        .model
        .boards()
        .iter()
        .find(|b| b.id == board_id)
        .cloned()
        .unwrap();
    let picker = SprintPickerView::for_card_assignment(app.model.sprints(), &board, None, now);
    let grid = render_picker_with_colors(&picker, Some(0));
    let color = line_color(&grid, "Active / Planned");
    assert_eq!(
        color,
        Some(Color::Yellow),
        "Active / Planned header must render in yellow"
    );
}

#[test]
fn test_render_with_none_selection_leaves_all_rows_unselected() {
    // Contract: selected=None means "no row highlighted". The (None) unassign
    // row must not gain the '> ' selected prefix just because index 0 happens
    // to coincide with the unwrap_or default. The primitive must stay neutral
    // for the None case so consumers control highlighting explicitly.
    let (mut app, board_id, _col) = make_app_with_board();
    add_active_sprint(&mut app, board_id);
    let now = Utc::now();
    let board = app
        .model
        .boards()
        .iter()
        .find(|b| b.id == board_id)
        .cloned()
        .unwrap();
    let picker = SprintPickerView::for_card_assignment(app.model.sprints(), &board, None, now);
    let out = render_picker_to_string(&picker, None);
    assert!(
        !out.contains("> (None)"),
        "with selected=None, no row should carry the '> ' selected prefix:\n{}",
        out
    );
}

#[test]
fn test_render_with_out_of_bounds_selected_does_not_panic() {
    // Defensive: stale or otherwise out-of-bounds `selected` must not panic
    // the render path. The sticky-header lookup loop iterates indices
    // beyond items.len() when selected is far past the end; it must clamp.
    let (mut app, board_id, _col) = make_app_with_board();
    add_active_sprint(&mut app, board_id);
    let now = Utc::now();
    let board = app
        .model
        .boards()
        .iter()
        .find(|b| b.id == board_id)
        .cloned()
        .unwrap();
    let picker = SprintPickerView::for_card_assignment(app.model.sprints(), &board, None, now);
    let out_of_bounds = picker.len() + 100;
    // Test passes if this render call returns instead of panicking.
    let _ = render_picker_to_string(&picker, Some(out_of_bounds));
}
