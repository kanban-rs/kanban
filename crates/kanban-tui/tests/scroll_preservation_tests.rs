use kanban_domain::{CreateCardOptions, KanbanOperations};
use kanban_tui::view_strategy::UnifiedViewStrategy;
use kanban_tui::App;

#[test]
fn test_column_view_scroll_offset_preserved_after_prepare_frame() {
    let mut app = App::test_default();

    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();

    for i in 0..20 {
        app.ctx
            .create_card(
                board.id,
                column.id,
                format!("Card {}", i),
                CreateCardOptions::default(),
            )
            .unwrap();
    }

    app.view.strategy = Box::new(UnifiedViewStrategy::kanban());
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.reload_model();
    app.prepare_frame();

    if let Some(list) = app.view.strategy.get_active_task_list_mut() {
        list.set_selected_index(Some(10));
        list.set_scroll_offset(5);
    }

    let offset_before = app
        .view
        .strategy
        .get_active_task_list()
        .unwrap()
        .get_scroll_offset();
    assert_eq!(offset_before, 5);

    app.reload_model();
    app.prepare_frame();

    let offset_after = app
        .view
        .strategy
        .get_active_task_list()
        .unwrap()
        .get_scroll_offset();
    assert_eq!(
        offset_after, 5,
        "scroll offset should be preserved after prepare_frame"
    );
}
