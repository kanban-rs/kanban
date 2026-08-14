//! KAN-394: verify that TUI multi-select batch handlers (`'c'`, `'h'`, `'l'`,
//! sprint-detail multi-toggle) route through the service-layer `update_cards`
//! API and inherit the status ↔ completion-column auto-sync, *including* the
//! per-batch position offset that gives chained moves into the same column
//! distinct positions instead of all colliding on the same one.
//!
//! Each test asserts:
//! 1. End state: every card ended up in the expected column with the expected status
//! 2. Position distinctness: chained moves into the same column produce 0, 1, 2 …
//!    instead of all sharing a single position
//! 3. Single undo unit: one `undo()` reverses every chained command across every
//!    card in the multi-select

use kanban_domain::{CardStatus, ColumnUpdate, CreateCardOptions, KanbanOperations};
use kanban_tui::app::focus::Focus;
use kanban_tui::App;

#[test]
fn test_multi_select_toggle_completion_batches_into_one_undo_unit_with_distinct_positions() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let backlog = app
        .ctx
        .create_column(board.id, "Backlog".to_string(), None)
        .unwrap();
    let _progress = app
        .ctx
        .create_column(board.id, "InProgress".to_string(), None)
        .unwrap();
    let done = app
        .ctx
        .create_column(board.id, "Done".to_string(), None)
        .unwrap();
    app.ctx
        .update_column(
            done.id,
            ColumnUpdate {
                default_status: Some(Some(CardStatus::Done)),
                ..Default::default()
            },
        )
        .unwrap();

    let cards: Vec<_> = (0..3)
        .map(|i| {
            app.ctx
                .create_card(
                    board.id,
                    backlog.id,
                    format!("Card {}", i),
                    CreateCardOptions::default(),
                )
                .unwrap()
        })
        .collect();

    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.focus.active = Focus::Cards;
    app.prepare_frame();

    for card in &cards {
        app.multi_select.selected_cards.insert(card.id);
    }
    app.multi_select.selection_mode_active = true;

    app.handle_toggle_card_completion();

    let mut positions = Vec::new();
    for card in &cards {
        let updated = app.ctx.get_card(card.id).unwrap().unwrap();
        assert_eq!(
            updated.column_id, done.id,
            "{} should be in completion column after multi-select 'c'",
            updated.title
        );
        assert_eq!(updated.status, CardStatus::Done);
        positions.push(updated.position);
    }
    positions.sort();
    assert_eq!(
        positions,
        vec![0, 1, 2],
        "chained moves into the completion column must produce distinct positions, \
         not all collapse onto the same one (this is the position-collision fix \
         that the service-layer `update_cards` provides via per-batch offsets)"
    );

    assert!(app.ctx.can_undo(), "batch should be one undo unit");
    assert!(app.ctx.undo().unwrap(), "undo should succeed");
    for card in &cards {
        let restored = app.ctx.get_card(card.id).unwrap().unwrap();
        assert_eq!(
            restored.column_id, backlog.id,
            "{} should be back in Backlog after a single undo",
            restored.title
        );
        assert_eq!(restored.status, CardStatus::Todo);
        assert!(restored.completed_at.is_none());
    }
}

#[test]
fn test_multi_select_move_right_to_completion_column_chains_status_per_card() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let backlog = app
        .ctx
        .create_column(board.id, "Backlog".to_string(), None)
        .unwrap();
    let done = app
        .ctx
        .create_column(board.id, "Done".to_string(), None)
        .unwrap();
    app.ctx
        .update_column(
            done.id,
            ColumnUpdate {
                default_status: Some(Some(CardStatus::Done)),
                ..Default::default()
            },
        )
        .unwrap();

    let cards: Vec<_> = (0..3)
        .map(|i| {
            app.ctx
                .create_card(
                    board.id,
                    backlog.id,
                    format!("Card {}", i),
                    CreateCardOptions::default(),
                )
                .unwrap()
        })
        .collect();

    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.focus.active = Focus::Cards;
    app.prepare_frame();

    for card in &cards {
        app.multi_select.selected_cards.insert(card.id);
    }
    app.multi_select.selection_mode_active = true;

    app.handle_move_card_right();

    let mut positions = Vec::new();
    for card in &cards {
        let updated = app.ctx.get_card(card.id).unwrap().unwrap();
        assert_eq!(
            updated.column_id, done.id,
            "{} should be in Done after multi-select 'l'",
            updated.title
        );
        assert_eq!(
            updated.status,
            CardStatus::Done,
            "{} status must auto-flip when crossing into completion column",
            updated.title
        );
        assert!(updated.completed_at.is_some());
        positions.push(updated.position);
    }
    positions.sort();
    assert_eq!(
        positions,
        vec![0, 1, 2],
        "chained moves into the completion column must produce distinct positions"
    );

    assert!(app.ctx.can_undo());
    assert!(app.ctx.undo().unwrap());
    for card in &cards {
        let restored = app.ctx.get_card(card.id).unwrap().unwrap();
        assert_eq!(restored.column_id, backlog.id);
        assert_eq!(restored.status, CardStatus::Todo);
        assert!(restored.completed_at.is_none());
    }
}
