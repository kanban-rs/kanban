use kanban_domain::KanbanOperations;
use kanban_service::test_helpers::FaultInjectingBackend;
use kanban_tui::App;
use std::sync::Arc;

#[test]
fn test_reload_model_keeps_the_controller_partitions_on_the_restored_model_when_a_read_fails() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    app.reload_model();

    assert_eq!(
        app.controller
            .displayed_boards(false)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|b| b.id)
            .collect::<Vec<_>>(),
        vec![board.id]
    );

    let fault = Arc::new(FaultInjectingBackend::new(app.ctx.backend()));
    app.ctx.replace_backend(fault.clone());
    fault.fail("list_boards");

    app.reload_model();

    assert!(app.ui_state.banner.is_some());
    assert_eq!(
        app.controller
            .displayed_boards(false)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|b| b.id)
            .collect::<Vec<_>>(),
        vec![board.id]
    );
}
