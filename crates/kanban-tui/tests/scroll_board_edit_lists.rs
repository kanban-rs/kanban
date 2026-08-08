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

#[test]
fn test_board_sprints_list_scrolls_to_keep_selection_visible() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    for i in 0..20 {
        app.ctx
            .create_sprint(board.id, None, Some(format!("SpMark{:02}", i)))
            .unwrap();
    }
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.push_mode(AppMode::BoardDetail);
    app.focus.board_focus = BoardFocus::Sprints;
    app.selection.sprint.set(Some(19));

    let output = render_to_string(&mut app, 80, 30);

    assert!(
        output.contains("SpMark19"),
        "selected sprint (last one) should be visible after scroll, got:\n{}",
        output
    );
    assert!(
        !output.contains("SpMark00"),
        "first sprint should have scrolled off-screen, got:\n{}",
        output
    );
}

#[test]
fn test_board_columns_list_scrolls_to_keep_selection_visible() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    for i in 0..20 {
        app.ctx
            .create_column(board.id, format!("ColMark{:02}", i), Some(i))
            .unwrap();
    }
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.push_mode(AppMode::BoardDetail);
    app.focus.board_focus = BoardFocus::Columns;
    app.dialog_input.column_selection.set(Some(19));

    let output = render_to_string(&mut app, 80, 30);

    assert!(
        output.contains("ColMark19"),
        "selected column (last one) should be visible after scroll, got:\n{}",
        output
    );
    assert!(
        !output.contains("ColMark00"),
        "first column should have scrolled off-screen, got:\n{}",
        output
    );
}

#[test]
fn test_board_sprints_scroll_offset_is_stable_when_selection_moves_within_viewport() {
    // Once scrolled, navigating one item up keeps the same scroll offset:
    // the previously-selected item stays visible, only the cursor moves.
    // This is the minimal-scroll semantics shared with the main card list.
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    for i in 0..20 {
        app.ctx
            .create_sprint(board.id, None, Some(format!("StaySp{:02}", i)))
            .unwrap();
    }
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.push_mode(AppMode::BoardDetail);
    app.focus.board_focus = BoardFocus::Sprints;

    app.selection.sprint.set(Some(19));
    render_to_string(&mut app, 80, 30);
    let offset_after_jump_down = app.selection.sprint_scroll.get();
    assert!(
        offset_after_jump_down > 0,
        "expected non-zero scroll after selecting last sprint, got {}",
        offset_after_jump_down
    );

    app.selection.sprint.set(Some(18));
    render_to_string(&mut app, 80, 30);
    assert_eq!(
        app.selection.sprint_scroll.get(),
        offset_after_jump_down,
        "scroll offset should remain stable when selection moves within viewport"
    );
}

#[test]
fn test_filter_popup_sprints_scrolls_to_keep_selection_visible() {
    use kanban_domain::CardFilters;
    use kanban_view::filters::FilterDialogState;

    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    for i in 0..20 {
        app.ctx
            .create_sprint(board.id, None, Some(format!("FpMark{:02}", i)))
            .unwrap();
    }
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.push_mode(AppMode::Dialog(DialogMode::FilterOptions));
    let mut dialog = FilterDialogState::new(CardFilters::default());
    // item_selection 0 = "show unassigned" row, 1..=20 = sprints index-0..19
    // Select the last sprint (index 19 in the sprint list).
    dialog.item_selection = 20;
    app.filter.dialog_state = Some(dialog);

    let output = render_to_string(&mut app, 80, 30);

    assert!(
        output.contains("FpMark19"),
        "selected sprint (last in filter popup) should be visible after scroll, got:\n{}",
        output
    );
    assert!(
        !output.contains("FpMark00"),
        "first sprint in filter popup should have scrolled off-screen, got:\n{}",
        output
    );
}
