use super::*;
use ratatui::layout::Rect;

#[test]
fn test_slash_key_activates_search_when_focus_is_boards() {
    let mut app = App::test_default();
    app.focus.active = Focus::Boards;

    app.handle_normal_key(crossterm::event::KeyCode::Char('/'));

    assert_eq!(app.mode, AppMode::Search);
    assert!(app.filter.board_search.is_active);
    assert!(
        !app.filter.search.is_active,
        "activating board search must not also activate card search"
    );
}

#[test]
fn test_scroll_help_into_view_scrolls_deep_item() {
    let mut app = App::test_default();
    app.view.last_frame_area = Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 50,
    };
    app.ui_state.help_list.update_item_count(50);
    app.ui_state.help_list.jump_to(49);
    app.scroll_help_into_view();
    assert!(
        app.ui_state.help_list.get_scroll_offset() > 0,
        "help list should have scrolled to bring item 49 into view"
    );
}
