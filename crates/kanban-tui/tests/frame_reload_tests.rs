mod helpers;

use helpers::CountingBackend;
use kanban_domain::KanbanOperations;
use kanban_tui::app::focus::Focus;
use kanban_tui::App;
use std::sync::atomic::Ordering;

fn wrap_backend(app: &mut App) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
    let (backend, reads) = CountingBackend::wrap(app.ctx.backend());
    app.ctx.replace_backend(backend);
    reads
}

#[test]
fn test_redraw_without_input_performs_no_store_reads() {
    let mut app = App::test_default();
    app.ctx.create_board("Board".to_string(), None).unwrap();
    app.reload_model();
    app.prepare_frame();

    let reads = wrap_backend(&mut app);

    app.needs_redraw = true;
    app.prepare_frame();

    assert_eq!(
        reads.load(Ordering::SeqCst),
        0,
        "redrawing without new input must not touch the store"
    );
}

#[test]
fn test_navigation_performs_no_store_reads() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    for i in 0..3 {
        app.ctx
            .create_card(
                board.id,
                column.id,
                format!("Card {i}"),
                kanban_domain::CreateCardOptions::default(),
            )
            .unwrap();
    }
    app.reload_model();
    app.prepare_frame();

    let reads = wrap_backend(&mut app);

    app.focus.active = Focus::Boards;
    app.handle_selection_activate();
    app.handle_navigation_down();
    app.handle_navigation_up();
    app.handle_kanban_column_left();
    app.handle_kanban_column_right();
    app.handle_focus_switch(Focus::Boards);
    app.handle_jump_to_top();
    app.handle_jump_to_bottom();
    app.handle_toggle_archived_boards_view();
    app.handle_toggle_archived_boards_view();

    assert_eq!(
        reads.load(Ordering::SeqCst),
        0,
        "pure navigation must not touch the store"
    );
}

#[test]
fn test_card_mutation_is_visible_in_model_without_a_further_redraw() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    app.ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    app.selection.active_board_id = Some(board.id);
    app.reload_model();
    app.prepare_frame();

    app.focus.active = Focus::Cards;
    app.input = kanban_core::InputState::new();
    app.input.set("New card".to_string());
    app.create_card();

    let title_present = app.model.all_cards().iter().any(|c| c.title == "New card");
    assert!(
        title_present,
        "create_card's own reload_model must make the new card visible without a further redraw"
    );
}
