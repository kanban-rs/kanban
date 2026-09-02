use kanban_domain::KanbanOperations;
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::AppMode;
use kanban_tui::App;

#[test]
fn test_escape_clears_a_committed_board_search_without_leaving_the_active_board() {
    let mut app = App::test_default();
    let alpha = app
        .ctx
        .create_board("Alpha Project".to_string(), None)
        .unwrap();
    app.ctx
        .create_board("Beta Project".to_string(), None)
        .unwrap();
    app.reload_model();
    app.prepare_frame();

    app.focus.active = Focus::Boards;
    app.selection.active_board_id = Some(alpha.id);
    app.filter.board_search.activate();
    for c in "alpha".chars() {
        app.filter.board_search.input.insert_char(c);
    }
    app.push_mode(AppMode::Search);
    app.handle_search_mode(crossterm::event::KeyCode::Enter);

    app.handle_escape_key();

    assert!(!app.filter.board_search.is_active);
    assert!(app.filter.board_search.query().is_empty());
    assert_eq!(
        app.displayed_boards()
            .loaded()
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .len(),
        2
    );
    assert_eq!(app.selection.active_board_id, Some(alpha.id));
}

#[test]
fn test_escape_clears_a_column_search_left_active_in_normal_mode() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    app.reload_model();
    app.prepare_frame();

    app.selection.active_board_id = Some(board.id);
    app.filter.column_search.activate();
    for c in "todo".chars() {
        app.filter.column_search.input.insert_char(c);
    }

    app.handle_escape_key();

    assert!(!app.filter.column_search.is_active);
    assert!(app.filter.column_search.query().is_empty());
    assert_eq!(app.selection.active_board_id, Some(board.id));
}

#[test]
fn test_escape_still_clears_a_committed_card_search() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    app.reload_model();
    app.prepare_frame();

    app.selection.active_board_id = Some(board.id);
    app.filter.search.activate();
    app.filter.search.input.insert_char('x');

    app.handle_escape_key();

    assert!(!app.filter.search.is_active);
    assert_eq!(app.selection.active_board_id, Some(board.id));
}

#[test]
fn test_escape_without_an_active_search_still_exits_selection_mode() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    app.reload_model();
    app.prepare_frame();

    app.selection.active_board_id = Some(board.id);
    app.multi_select.selection_mode_active = true;
    app.multi_select.selected_cards.insert(uuid::Uuid::new_v4());

    app.handle_escape_key();

    assert!(!app.multi_select.selection_mode_active);
    assert!(app.multi_select.selected_cards.is_empty());
    assert_eq!(app.selection.active_board_id, Some(board.id));
}

#[test]
fn test_escape_with_no_search_or_selection_still_leaves_the_active_board() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    app.reload_model();
    app.prepare_frame();

    app.selection.active_board_id = Some(board.id);

    app.handle_escape_key();

    assert_eq!(app.selection.active_board_id, None);
    assert_eq!(app.focus.active, Focus::Boards);
}
