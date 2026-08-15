mod helpers;

use helpers::SnapshotCountingBackend;
use kanban_domain::{AnimationType, CreateCardOptions, KanbanOperations};
use kanban_tui::app::animation::CardAnimation;
use kanban_tui::app::focus::Focus;
use kanban_tui::App;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

fn wrap_backend(app: &mut App) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
    let (backend, reads) = SnapshotCountingBackend::wrap(app.ctx.backend());
    app.ctx.replace_backend(backend);
    reads
}

fn insert_completed_animation(app: &mut App, card_id: uuid::Uuid, animation_type: AnimationType) {
    app.animation.animating.insert(
        card_id,
        CardAnimation {
            animation_type,
            start_time: Instant::now() - Duration::from_millis(200),
        },
    );
}

#[test]
fn test_animation_tick_completing_archive_and_delete_reads_the_store_once() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    let live_card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Live".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let archived_card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Archived".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.archive_card(archived_card.id).unwrap();
    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Cards;
    app.reload_model();
    app.prepare_frame();

    insert_completed_animation(&mut app, live_card.id, AnimationType::Archiving);
    insert_completed_animation(&mut app, archived_card.id, AnimationType::Deleting);

    let reads = wrap_backend(&mut app);
    app.handle_animation_tick();

    assert_eq!(
        reads.load(Ordering::SeqCst),
        1,
        "a tick completing both an archive and a delete must reload exactly once"
    );
}

#[test]
fn test_animation_tick_with_only_archives_still_reads_once() {
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
            "Live".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Cards;
    app.reload_model();
    app.prepare_frame();

    insert_completed_animation(&mut app, card.id, AnimationType::Archiving);

    let reads = wrap_backend(&mut app);
    app.handle_animation_tick();

    assert_eq!(
        reads.load(Ordering::SeqCst),
        1,
        "a tick completing only archives must still reload exactly once"
    );
}

#[test]
fn test_animation_tick_with_a_failed_batch_does_not_reload() {
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
            "Live".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Cards;
    app.reload_model();
    app.prepare_frame();

    insert_completed_animation(&mut app, card.id, AnimationType::Archiving);
    // Delete the card out from under the pending archive animation so its
    // batch fails at execution time (card not found).
    app.ctx.data_store().delete_card(card.id).unwrap();

    let selected_before = app.get_selected_card_id();
    let reads = wrap_backend(&mut app);
    app.handle_animation_tick();

    assert_eq!(
        reads.load(Ordering::SeqCst),
        0,
        "a failed batch must not reload the model"
    );
    assert_eq!(
        app.get_selected_card_id(),
        selected_before,
        "a failed batch must not change the selection"
    );
}

#[test]
fn test_animation_tick_archive_selects_the_following_card() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    let first = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "First".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let second = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Second".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Cards;
    app.reload_model();
    app.prepare_frame();

    app.animation.archive_anchor = Some((column.id, first.position));
    insert_completed_animation(&mut app, first.id, AnimationType::Archiving);

    app.handle_animation_tick();

    assert_eq!(
        app.get_selected_card_id(),
        Some(second.id),
        "after archiving the first card, selection must land on the following card"
    );
}

#[test]
fn test_animation_tick_restoring_many_cards_reads_the_store_once() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    let mut cards = Vec::new();
    for i in 0..5 {
        let card = app
            .ctx
            .create_card(
                board.id,
                column.id,
                format!("Card {i}"),
                CreateCardOptions::default(),
            )
            .unwrap();
        app.ctx.archive_card(card.id).unwrap();
        cards.push(card);
    }
    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Cards;
    app.reload_model();
    app.prepare_frame();

    for card in &cards {
        insert_completed_animation(&mut app, card.id, AnimationType::Restoring);
    }

    let reads = wrap_backend(&mut app);
    app.handle_animation_tick();

    assert_eq!(
        reads.load(Ordering::SeqCst),
        1,
        "restoring several cards in one tick must reload exactly once, not once per card"
    );
}

#[test]
fn test_animation_tick_restoring_many_cards_puts_each_in_the_right_column() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column_a = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    let column_b = app
        .ctx
        .create_column(board.id, "Doing".to_string(), Some(1))
        .unwrap();
    let card_a = app
        .ctx
        .create_card(
            board.id,
            column_a.id,
            "A".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let card_b = app
        .ctx
        .create_card(
            board.id,
            column_b.id,
            "B".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.archive_card(card_a.id).unwrap();
    app.ctx.archive_card(card_b.id).unwrap();
    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Cards;
    app.reload_model();
    app.prepare_frame();

    insert_completed_animation(&mut app, card_a.id, AnimationType::Restoring);
    insert_completed_animation(&mut app, card_b.id, AnimationType::Restoring);

    app.handle_animation_tick();

    let restored_a = app.model.card_by_id(card_a.id).expect("card A restored");
    let restored_b = app.model.card_by_id(card_b.id).expect("card B restored");
    assert_eq!(
        restored_a.column_id, column_a.id,
        "card A must be restored to its own column"
    );
    assert_eq!(
        restored_b.column_id, column_b.id,
        "card B must be restored to its own column"
    );
    assert_eq!(restored_a.position, card_a.position);
    assert_eq!(restored_b.position, card_b.position);
}

#[test]
fn test_animation_tick_mixed_archive_delete_restore_reads_the_store_once() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    let live_card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Live".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let to_delete = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "ToDelete".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.archive_card(to_delete.id).unwrap();
    let to_restore = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "ToRestore".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.archive_card(to_restore.id).unwrap();
    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Cards;
    app.reload_model();
    app.prepare_frame();

    insert_completed_animation(&mut app, live_card.id, AnimationType::Archiving);
    insert_completed_animation(&mut app, to_delete.id, AnimationType::Deleting);
    insert_completed_animation(&mut app, to_restore.id, AnimationType::Restoring);

    let reads = wrap_backend(&mut app);
    app.handle_animation_tick();

    assert_eq!(
        reads.load(Ordering::SeqCst),
        1,
        "a tick completing all three animation kinds must still reload exactly once"
    );
}

#[test]
fn test_animation_tick_archive_and_delete_are_separate_undo_entries() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    let live_card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Live".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let archived_card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Archived".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.archive_card(archived_card.id).unwrap();
    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Cards;
    app.reload_model();
    app.prepare_frame();

    insert_completed_animation(&mut app, live_card.id, AnimationType::Archiving);
    insert_completed_animation(&mut app, archived_card.id, AnimationType::Deleting);

    app.handle_animation_tick();

    // Both completed in the same tick: the live card was archived, and the
    // already-archived card was permanently deleted.
    assert!(app.model.live_cards().iter().all(|c| c.id != live_card.id));
    assert!(app.model.card_by_id(archived_card.id).is_none());

    // One `u` press must revert exactly one of those two user actions, not
    // both at once.
    app.undo().unwrap();

    // Exactly one of the two user actions must have been reverted by the
    // single undo, never both and never neither.
    let archive_reverted = app.model.live_cards().iter().any(|c| c.id == live_card.id);
    let delete_reverted = app.model.card_by_id(archived_card.id).is_some();
    assert!(
        archive_reverted ^ delete_reverted,
        "a single undo must revert exactly one of the two batched user actions"
    );
}
