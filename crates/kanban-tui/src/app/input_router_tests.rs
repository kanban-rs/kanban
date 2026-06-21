use super::*;
use ratatui::layout::Rect;

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
