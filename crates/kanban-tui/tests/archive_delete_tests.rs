mod helpers;

use helpers::{warm_archived_card_markers, CountingBackend};
use kanban_domain::{CreateCardOptions, KanbanOperations, UndoOperations};
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::AppMode;
use kanban_tui::App;
use std::time::{Duration, Instant};

fn force_animation_complete(app: &mut App, card_id: uuid::Uuid) {
    app.animation
        .animating
        .get_mut(&card_id)
        .unwrap()
        .start_time = Instant::now() - Duration::from_millis(200);
}

#[test]
fn test_archived_card_visible_via_card_by_id() {
    let mut app = App::test_default();

    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "ArchiveMe".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let card_id = card.id;

    app.ctx.archive_card(card_id).unwrap();

    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.mode = AppMode::ArchivedCardsView;
    app.reload_model();
    app.prepare_frame();

    let found = app.model.card_by_id_state(card_id).loaded().copied();
    assert!(
        found.is_some(),
        "card_by_id_state should return archived card, got None"
    );
    assert_eq!(found.unwrap().title, "ArchiveMe");
}

#[test]
fn test_archived_card_appears_in_task_list() {
    let mut app = App::test_default();

    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Archived".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();

    app.ctx.archive_card(card.id).unwrap();

    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.mode = AppMode::ArchivedCardsView;
    app.reload_model();
    app.prepare_frame();

    let list = app.view.strategy.get_active_task_list();
    assert!(list.is_some(), "active task list should exist");
    assert_eq!(
        list.unwrap().len(),
        1,
        "task list should contain the archived card"
    );
}

#[test]
fn test_permanent_delete_removes_archived_card() {
    let mut app = App::test_default();

    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "DeleteMe".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let card_id = card.id;

    app.ctx.archive_card(card_id).unwrap();

    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.mode = AppMode::ArchivedCardsView;
    app.reload_model();
    app.prepare_frame();

    if let Some(list) = app.view.strategy.get_active_task_list_mut() {
        list.set_selected_index(Some(0));
    }

    app.handle_delete_card_permanent();

    assert!(
        app.animation.animating.contains_key(&card_id),
        "animation should have started for the card"
    );

    let anim = app.animation.animating.get_mut(&card_id).unwrap();
    anim.start_time = Instant::now() - Duration::from_millis(200);

    app.handle_animation_tick();

    app.reload_model();
    app.prepare_frame();

    assert!(
        app.model.archived_card_markers().is_empty(),
        "archived cards should be empty after permanent delete"
    );
    assert!(
        app.model
            .column_cards_state(column.id)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .all(|c| c.id != card_id),
        "card should not be restored to active cards"
    );
    assert!(
        app.model
            .card_by_id_state(card_id)
            .loaded()
            .copied()
            .is_none(),
        "card_by_id_state should return None for permanently deleted card"
    );
}

/// A single archive (the user-perceived "delete") must produce exactly one
/// undo step. Previously the archive flow emitted two batches — `ArchiveCards`
/// followed by `CompactColumnPositions` — so the first `u` only reverted the
/// position-compact and the card stayed archived.
#[test]
fn test_archive_animation_completion_is_a_single_undo_step() {
    let mut app = App::test_default();

    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "ArchiveMe".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let card_id = card.id;

    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.reload_model();
    app.prepare_frame();

    app.start_delete_animation(card_id);
    force_animation_complete(&mut app, card_id);
    app.handle_animation_tick();
    app.reload_model();
    app.prepare_frame();
    warm_archived_card_markers(&mut app);

    // Unified model: the row stays reachable via `card_by_id_state`; archival
    // is recorded by the id set. "Archived" means present in
    // `archived_card_ids`, not removed.
    assert!(
        app.model.card_by_id_state(card_id).is_loaded()
            && app.model.archived_card_ids().contains(&card_id),
        "card must be archived (marked) after animation completion"
    );

    assert!(app.ctx.undo().unwrap().is_some(), "first undo must succeed");
    app.reload_model();
    app.prepare_frame();
    warm_archived_card_markers(&mut app);

    assert!(
        app.model
            .board_cards_state(board.id)
            .loaded()
            .map(|v| v.iter().any(|c| c.id == card_id))
            .unwrap_or(false)
            && !app.model.archived_card_ids().contains(&card_id),
        "card must be live again after one undo press — archive + compact must \
         live in a single undo batch"
    );
}

/// Archiving cards from multiple columns at once must compact every affected
/// column atomically. Previously only `last_archive_column` was compacted, so
/// non-last columns kept a positional gap until the next compact ran.
#[test]
fn test_multi_column_archive_compacts_every_affected_column() {
    let mut app = App::test_default();

    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let col1 = app
        .ctx
        .create_column(board.id, "C1".to_string(), None)
        .unwrap();
    let col2 = app
        .ctx
        .create_column(board.id, "C2".to_string(), None)
        .unwrap();

    let archive1 = app
        .ctx
        .create_card(
            board.id,
            col1.id,
            "A1".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let keep1 = app
        .ctx
        .create_card(
            board.id,
            col1.id,
            "K1".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let archive2 = app
        .ctx
        .create_card(
            board.id,
            col2.id,
            "A2".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let keep2 = app
        .ctx
        .create_card(
            board.id,
            col2.id,
            "K2".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();

    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.reload_model();
    app.prepare_frame();

    app.start_delete_animation(archive1.id);
    app.start_delete_animation(archive2.id);
    force_animation_complete(&mut app, archive1.id);
    force_animation_complete(&mut app, archive2.id);
    app.handle_animation_tick();
    app.reload_model();
    app.prepare_frame();

    let cards = app.model.board_cards_state(board.id);
    let cards = cards.loaded().map(|v| v.as_slice()).unwrap_or(&[]);
    let k1 = cards.iter().find(|c| c.id == keep1.id).unwrap();
    let k2 = cards.iter().find(|c| c.id == keep2.id).unwrap();
    assert_eq!(
        k1.position, 0,
        "col1 must be compacted to position 0 after archiving its first card"
    );
    assert_eq!(
        k2.position, 0,
        "col2 must be compacted to position 0 after archiving its first card"
    );
}

/// When archiving cards from multiple columns at once, post-archive selection
/// must anchor to the column the user's cursor was on — not jump to whichever
/// column happened to be iterated last in HashMap order.
#[test]
fn test_archive_anchors_selection_to_focused_card_column() {
    let mut app = App::test_default();

    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let col1 = app
        .ctx
        .create_column(board.id, "C1".to_string(), None)
        .unwrap();
    let col2 = app
        .ctx
        .create_column(board.id, "C2".to_string(), None)
        .unwrap();

    let archive1 = app
        .ctx
        .create_card(
            board.id,
            col1.id,
            "A1".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let keep1 = app
        .ctx
        .create_card(
            board.id,
            col1.id,
            "K1".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let archive2 = app
        .ctx
        .create_card(
            board.id,
            col2.id,
            "A2".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx
        .create_card(
            board.id,
            col2.id,
            "K2".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();

    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.focus.active = Focus::Cards;
    app.reload_model();
    app.prepare_frame();

    // Multi-select cards from both columns; cursor on col1's archive target.
    app.multi_select.selected_cards.insert(archive1.id);
    app.multi_select.selected_cards.insert(archive2.id);
    app.multi_select.selection_mode_active = true;
    app.select_card_by_id(archive1.id);

    app.handle_archive_card();

    force_animation_complete(&mut app, archive1.id);
    force_animation_complete(&mut app, archive2.id);
    app.handle_animation_tick();
    app.reload_model();
    app.prepare_frame();

    assert_eq!(
        app.get_selected_card_id(),
        Some(keep1.id),
        "selection must anchor to the cursor's column (col1 → keep1), not \
         jump to col2 based on archive iteration order"
    );
}

#[test]
fn test_q_in_archived_view_returns_to_normal() {
    let mut app = App::test_default();

    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    app.ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();

    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.mode = AppMode::ArchivedCardsView;
    app.reload_model();
    app.prepare_frame();

    app.handle_archived_cards_view_mode(crossterm::event::KeyCode::Char('q'));

    assert_eq!(
        app.mode,
        AppMode::Normal,
        "pressing 'q' in ArchivedCardsView should return to Normal mode"
    );
    assert!(
        !app.should_quit,
        "pressing 'q' in ArchivedCardsView should not quit the app"
    );
}

/// `start_restore_animation`, `start_permanent_delete_animation` and
/// `complete_restore_animation` all check archived membership via
/// `board_archived_cards_state`, the by-board tier. A failure on the flat
/// `list_archived_cards` tier must not stop any of the three from working.
#[test]
fn test_restore_animation_detects_membership_when_the_global_archived_tier_fails() {
    let mut app = App::test_default();

    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "ArchiveMe".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let card_id = card.id;
    app.ctx.archive_card(card_id).unwrap();

    app.selection.active_board_id = Some(board.id);
    app.mode = AppMode::ArchivedCardsView;
    app.reload_model();
    app.prepare_frame();

    let failing = CountingBackend::wrap_failing(app.ctx.backend(), "list_archived_cards");
    app.ctx.replace_backend(failing);
    app.reload_model();
    app.prepare_frame();

    if let Some(list) = app.view.strategy.get_active_task_list_mut() {
        list.set_selected_index(Some(0));
    }

    app.handle_restore_card();

    assert!(
        app.animation.animating.contains_key(&card_id),
        "restore animation must start off the by-board archived tier even \
         when the global list_archived_cards tier is failing"
    );

    force_animation_complete(&mut app, card_id);
    app.handle_animation_tick();

    app.reload_model();
    app.prepare_frame();

    assert!(
        !app.model.archived_card_ids().contains(&card_id),
        "the card must actually be restored, proving complete_restore_animation \
         also resolved membership off the by-board tier"
    );
}

#[test]
fn test_permanent_delete_animation_detects_membership_when_the_global_archived_tier_fails() {
    let mut app = App::test_default();

    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "DeleteMe".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let card_id = card.id;
    app.ctx.archive_card(card_id).unwrap();

    app.selection.active_board_id = Some(board.id);
    app.mode = AppMode::ArchivedCardsView;
    app.reload_model();
    app.prepare_frame();

    let failing = CountingBackend::wrap_failing(app.ctx.backend(), "list_archived_cards");
    app.ctx.replace_backend(failing);
    app.reload_model();
    app.prepare_frame();

    if let Some(list) = app.view.strategy.get_active_task_list_mut() {
        list.set_selected_index(Some(0));
    }

    app.handle_delete_card_permanent();

    assert!(
        app.animation.animating.contains_key(&card_id),
        "permanent-delete animation must start off the by-board archived tier \
         even when the global list_archived_cards tier is failing"
    );
}

/// `start_restore_animation` and `start_permanent_delete_animation` both
/// gained a `scope_board_id()` guard when they moved onto the by-board
/// tier. With no board in scope, the guard must make them a deliberate
/// no-op rather than starting an animation with no board to resolve.
#[test]
fn test_no_board_in_scope_leaves_restore_and_permanent_delete_animations_unstarted() {
    let mut app = App::test_default();

    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "ArchiveMe".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let card_id = card.id;
    app.ctx.archive_card(card_id).unwrap();

    let board2 = app.ctx.create_board("Board 2".to_string(), None).unwrap();
    let column2 = app
        .ctx
        .create_column(board2.id, "Todo".to_string(), None)
        .unwrap();
    let card2 = app
        .ctx
        .create_card(
            board2.id,
            column2.id,
            "ArchiveMeToo".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let card2_id = card2.id;
    app.ctx.archive_card(card2_id).unwrap();

    app.selection.active_board_id = Some(board.id);
    app.mode = AppMode::ArchivedCardsView;
    app.refresh_view();

    app.selection.active_board_id = Some(board2.id);
    app.refresh_view();

    assert_eq!(
        app.model
            .board_archived_cards_state(board.id)
            .loaded()
            .map(|v| v.len()),
        Some(1),
        "fixture sanity: board 1's archived tier stays warm"
    );
    assert_eq!(
        app.model
            .board_archived_cards_state(board2.id)
            .loaded()
            .map(|v| v.len()),
        Some(1),
        "fixture sanity: board 2's archived tier is warm"
    );

    app.selection.active_board_id = None;
    app.board_list.inner_mut().set_selected_index(None);

    app.multi_select.selected_cards.insert(card_id);
    app.multi_select.selected_cards.insert(card2_id);
    app.handle_restore_card();
    assert!(
        !app.animation.animating.contains_key(&card_id),
        "restore must not start an animation for board 1's card with no board in scope"
    );
    assert!(
        !app.animation.animating.contains_key(&card2_id),
        "restore must not start an animation for board 2's card with no board in scope"
    );

    app.multi_select.selected_cards.insert(card_id);
    app.multi_select.selected_cards.insert(card2_id);
    app.handle_delete_card_permanent();
    assert!(
        !app.animation.animating.contains_key(&card_id),
        "permanent delete must not start an animation for board 1's card with no board in scope"
    );
    assert!(
        !app.animation.animating.contains_key(&card2_id),
        "permanent delete must not start an animation for board 2's card with no board in scope"
    );

    app.selection.active_board_id = Some(board2.id);
    app.multi_select.selected_cards.insert(card_id);
    app.multi_select.selected_cards.insert(card2_id);
    app.handle_restore_card();
    assert!(
        app.animation.animating.contains_key(&card2_id),
        "restore must start an animation for board 2's card once board 2 is back in scope"
    );
    assert!(
        !app.animation.animating.contains_key(&card_id),
        "board 1's card must not animate off board 2's scope"
    );

    app.animation.animating.clear();
    app.multi_select.selected_cards.insert(card_id);
    app.multi_select.selected_cards.insert(card2_id);
    app.handle_delete_card_permanent();
    assert!(
        app.animation.animating.contains_key(&card2_id),
        "permanent delete must start an animation for board 2's card once board 2 is back in scope"
    );
    assert!(
        !app.animation.animating.contains_key(&card_id),
        "board 1's card must not animate a permanent delete off board 2's scope"
    );
}
