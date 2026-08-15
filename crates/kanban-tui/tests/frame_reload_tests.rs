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

#[test]
fn test_archive_card_is_visible_in_model_without_a_further_redraw() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Card".to_string(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    app.selection.active_board_id = Some(board.id);
    app.reload_model();
    app.prepare_frame();

    app.focus.active = Focus::Cards;
    app.start_delete_animation(card.id);
    app.animation
        .animating
        .get_mut(&card.id)
        .unwrap()
        .start_time = std::time::Instant::now() - std::time::Duration::from_secs(10);
    app.handle_animation_tick();

    let still_live = app.model.live_cards().iter().any(|c| c.id == card.id);
    assert!(
        !still_live,
        "handle_animation_tick's own reload_model must remove the archived card from the live model without a further redraw"
    );
}

#[test]
fn test_restore_card_is_visible_in_model_without_a_further_redraw() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Card".to_string(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.archive_card(card.id).unwrap();
    app.selection.active_board_id = Some(board.id);
    app.reload_model();
    app.prepare_frame();

    let archived_card = app
        .model
        .archived_card_markers()
        .iter()
        .find(|dc| dc.entity_id == card.id)
        .cloned()
        .unwrap();
    app.restore_card(archived_card);

    let now_live = app.model.live_cards().iter().any(|c| c.id == card.id);
    assert!(
        now_live,
        "restore_card's own reload_model must make the restored card visible in the live model without a further redraw"
    );
}

#[test]
fn test_move_card_between_columns_is_visible_in_model_without_a_further_redraw() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let col_a = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    let col_b = app
        .ctx
        .create_column(board.id, "Doing".to_string(), Some(1))
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            col_a.id,
            "Card".to_string(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    app.selection.active_board_id = Some(board.id);
    app.reload_model();
    app.prepare_frame();
    app.select_card_by_id(card.id);

    app.focus.active = Focus::Cards;
    app.handle_move_card_right();

    let moved = app
        .model
        .card_by_id(card.id)
        .map(|c| c.column_id == col_b.id)
        .unwrap_or(false);
    assert!(
        moved,
        "handle_move_card_right's own reload_model must make the moved card's new column visible without a further redraw"
    );
}

#[test]
fn test_sprint_assignment_is_visible_in_model_without_a_further_redraw() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Card".to_string(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let sprint = app.ctx.create_sprint(board.id, None, None).unwrap();
    app.selection.active_board_id = Some(board.id);
    app.selection.active_card_id = Some(card.id);
    app.reload_model();
    app.prepare_frame();

    app.dialog_input
        .assign_sprint_picker
        .reset_for_card_assignment(
            Some(sprint.id),
            app.model.sprints(),
            app.model.board_by_id(board.id).unwrap(),
            chrono::Utc::now(),
        );
    app.handle_assign_card_to_sprint_popup(crossterm::event::KeyCode::Enter);

    let assigned = app
        .model
        .card_by_id(card.id)
        .and_then(|c| c.sprint_id)
        .map(|id| id == sprint.id)
        .unwrap_or(false);
    assert!(
        assigned,
        "handle_assign_card_to_sprint_popup must make the card's sprint assignment visible without a further redraw"
    );
}

#[test]
fn test_delete_board_leaves_no_stale_model_without_guard_reload() {
    let mut app = App::test_default();
    let b = app.ctx.create_board("Doomed".to_string(), None).unwrap();
    app.reload_model();
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = Some(b.id);
    assert_eq!(
        app.displayed_boards().len(),
        1,
        "precondition: board visible"
    );

    app.delete_board();
    let live_in_store = app.ctx.data_store().list_boards().unwrap().len();
    app.prepare_frame();
    let visible_after_redraw = app.displayed_boards().len();

    assert_eq!(live_in_store, 0, "store: board is gone");
    assert_eq!(
        visible_after_redraw, 0,
        "UI STALE: store has {live_in_store} live boards but the panel still shows {visible_after_redraw}"
    );
}
