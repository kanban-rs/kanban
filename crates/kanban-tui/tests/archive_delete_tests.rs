use kanban_domain::{CreateCardOptions, KanbanOperations};
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
    app.prepare_frame();

    let found = app.model.card_by_id(card_id);
    assert!(
        found.is_some(),
        "card_by_id should return archived card, got None"
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

    app.prepare_frame();

    assert!(
        app.model.archived_cards().is_empty(),
        "archived cards should be empty after permanent delete"
    );
    assert!(
        app.model.cards().iter().all(|c| c.id != card_id),
        "card should not be restored to active cards"
    );
    assert!(
        app.model.card_by_id(card_id).is_none(),
        "card_by_id should return None for permanently deleted card"
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
    app.prepare_frame();

    app.start_delete_animation(card_id);
    force_animation_complete(&mut app, card_id);
    app.handle_animation_tick();
    app.prepare_frame();

    // Unified model: the row stays in `cards()`; archival is recorded by the
    // id set. "Archived" means present in `archived_card_ids`, not removed.
    assert!(
        app.model.cards().iter().any(|c| c.id == card_id)
            && app.model.archived_card_ids().contains(&card_id),
        "card must be archived (marked) after animation completion"
    );

    assert!(app.ctx.undo().unwrap(), "first undo must succeed");
    app.prepare_frame();

    assert!(
        app.model.cards().iter().any(|c| c.id == card_id)
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
    app.prepare_frame();

    app.start_delete_animation(archive1.id);
    app.start_delete_animation(archive2.id);
    force_animation_complete(&mut app, archive1.id);
    force_animation_complete(&mut app, archive2.id);
    app.handle_animation_tick();
    app.prepare_frame();

    let cards = app.model.cards();
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
