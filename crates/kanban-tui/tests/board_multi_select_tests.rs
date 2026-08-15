use kanban_domain::KanbanOperations;
use kanban_tui::app::focus::Focus;
use kanban_tui::App;

fn create_boards(app: &mut App, names: &[&str]) -> Vec<uuid::Uuid> {
    names
        .iter()
        .map(|name| app.ctx.create_board((*name).to_string(), None).unwrap().id)
        .collect()
}

#[test]
fn test_handle_board_selection_toggle_enters_selection_mode_and_selects_current_board() {
    let mut app = App::test_default();
    let ids = create_boards(&mut app, &["Alpha", "Beta"]);
    app.reload_model();
    app.prepare_frame();
    app.focus.active = Focus::Boards;
    app.board_list.select_board(ids[0]);

    app.handle_board_selection_toggle();

    assert!(app.multi_select.board_selection_mode_active);
    assert!(app.multi_select.selected_boards.contains(&ids[0]));
    assert_eq!(app.multi_select.selected_boards.len(), 1);
}

#[test]
fn test_handle_board_selection_toggle_exits_selection_mode_keeps_selections() {
    let mut app = App::test_default();
    let ids = create_boards(&mut app, &["Alpha", "Beta"]);
    app.reload_model();
    app.prepare_frame();
    app.focus.active = Focus::Boards;
    app.board_list.select_board(ids[0]);

    app.handle_board_selection_toggle();
    assert!(app.multi_select.board_selection_mode_active);

    app.handle_board_selection_toggle();

    assert!(!app.multi_select.board_selection_mode_active);
    assert!(
        app.multi_select.selected_boards.contains(&ids[0]),
        "exiting selection mode must not clear prior selections"
    );
}

#[test]
fn test_handle_clear_board_selection_empties_selected_boards() {
    let mut app = App::test_default();
    let ids = create_boards(&mut app, &["Alpha", "Beta"]);
    app.multi_select.selected_boards.insert(ids[0]);
    app.multi_select.selected_boards.insert(ids[1]);

    app.handle_clear_board_selection();

    assert!(app.multi_select.selected_boards.is_empty());
}

#[test]
fn test_handle_select_all_boards_in_view_selects_only_currently_filtered_boards() {
    let mut app = App::test_default();
    let ids = create_boards(&mut app, &["Alpha Project", "Beta Project", "Alpha Two"]);
    app.focus.active = Focus::Boards;

    app.filter.board_search.activate();
    app.filter.board_search.input.set("Alpha".to_string());
    app.reload_model();
    app.prepare_frame();

    app.handle_select_all_boards_in_view();

    assert!(app.multi_select.selected_boards.contains(&ids[0]));
    assert!(app.multi_select.selected_boards.contains(&ids[2]));
    assert!(
        !app.multi_select.selected_boards.contains(&ids[1]),
        "select-all-in-view must not pull in boards excluded by the active search filter"
    );
    assert_eq!(app.multi_select.selected_boards.len(), 2);
    assert!(app.multi_select.board_selection_mode_active);
}

#[test]
fn test_handle_select_all_boards_in_view_does_nothing_when_focus_is_not_boards() {
    let mut app = App::test_default();
    create_boards(&mut app, &["Alpha", "Beta"]);
    app.reload_model();
    app.prepare_frame();
    app.focus.active = Focus::Cards;

    app.handle_select_all_boards_in_view();

    assert!(app.multi_select.selected_boards.is_empty());
    assert!(!app.multi_select.board_selection_mode_active);
}
