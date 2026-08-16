//! `handle_manage_children_from_list`, `handle_manage_parents`, and
//! `handle_manage_children` each build their eligible-candidates list from
//! the unified live+archived card collection with no archived exclusion, so
//! managing relationships from a LIVE card currently offers archived cards
//! as candidates too. Per KAN-972's resolution: a live target should default
//! to live-only candidates; an archived target (ArchivedCardsView's `s` key)
//! keeps full parity and still offers archived candidates.

use kanban_domain::{CreateCardOptions, KanbanOperations, Snapshot};
use kanban_tui::app::mode::AppMode;
use kanban_tui::App;

fn sync_model_from_store(app: &mut App) {
    let snapshot = Snapshot {
        archived_boards: app.ctx.data_store().list_archived_boards().unwrap(),
        boards: app.ctx.data_store().list_boards().unwrap(),
        columns: app.ctx.data_store().list_all_columns().unwrap(),
        cards: app.ctx.data_store().list_all_cards().unwrap(),
        archived_cards: app.ctx.data_store().list_archived_cards().unwrap(),
        sprints: app.ctx.data_store().list_all_sprints().unwrap(),
        graph: app.ctx.data_store().get_graph().unwrap(),
        prefixes: Vec::new(),
    };
    app.model.load_from_snapshot(snapshot);
}

/// Seed a board/column with a target card plus a live and an archived
/// candidate, all otherwise eligible (same board, no ancestor/descendant
/// relation to the target).
fn seed_target_and_candidates(app: &mut App) -> (uuid::Uuid, uuid::Uuid, uuid::Uuid, uuid::Uuid) {
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();

    let target = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Target".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let live_candidate = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "LiveCandidate".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let archived_candidate = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "ArchivedCandidate".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.archive_card(archived_candidate.id).unwrap();

    (
        board.id,
        target.id,
        live_candidate.id,
        archived_candidate.id,
    )
}

#[test]
fn test_manage_children_from_live_card_excludes_archived_candidates() {
    let mut app = App::test_default();
    let (board_id, target_id, live_id, archived_id) = seed_target_and_candidates(&mut app);
    sync_model_from_store(&mut app);

    app.selection.active_board_id = Some(board_id);
    app.reload_model();
    app.prepare_frame();
    let list = app.view.strategy.get_active_task_list_mut().unwrap();
    let idx = list
        .cards
        .iter()
        .position(|&id| id == target_id)
        .expect("target card is in the active task list");
    list.set_selected_index(Some(idx));

    app.handle_manage_children_from_list();

    assert!(
        app.relationship.card_ids.contains(&live_id),
        "live candidate must be offered"
    );
    assert!(
        !app.relationship.card_ids.contains(&archived_id),
        "archived candidate must be excluded when the target is live"
    );
}

#[test]
fn test_manage_children_from_archived_card_still_offers_archived_candidates() {
    let mut app = App::test_default();
    let (board_id, target_id, live_id, archived_id) = seed_target_and_candidates(&mut app);
    app.ctx.archive_card(target_id).unwrap();
    sync_model_from_store(&mut app);

    app.selection.active_board_id = Some(board_id);
    app.mode = AppMode::ArchivedCardsView;
    app.reload_model();
    app.prepare_frame();
    let list = app.view.strategy.get_active_task_list_mut().unwrap();
    let idx = list
        .cards
        .iter()
        .position(|&id| id == target_id)
        .expect("archived target card is in the archived task list");
    list.set_selected_index(Some(idx));

    app.handle_manage_children_from_list();

    assert!(
        app.relationship.card_ids.contains(&live_id),
        "live candidate must still be offered"
    );
    assert!(
        app.relationship.card_ids.contains(&archived_id),
        "archived candidate must still be offered when the target itself is archived (parity)"
    );
}

#[test]
fn test_manage_children_from_live_card_still_offers_live_candidates() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let target = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Target".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let live_a = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "LiveA".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let live_b = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "LiveB".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    sync_model_from_store(&mut app);

    app.selection.active_board_id = Some(board.id);
    app.reload_model();
    app.prepare_frame();
    let list = app.view.strategy.get_active_task_list_mut().unwrap();
    let idx = list
        .cards
        .iter()
        .position(|&id| id == target.id)
        .expect("target card is in the active task list");
    list.set_selected_index(Some(idx));

    app.handle_manage_children_from_list();

    assert!(app.relationship.card_ids.contains(&live_a.id));
    assert!(app.relationship.card_ids.contains(&live_b.id));
}

#[test]
fn test_card_detail_manage_children_excludes_archived_candidates() {
    let mut app = App::test_default();
    let (_board_id, target_id, live_id, archived_id) = seed_target_and_candidates(&mut app);
    sync_model_from_store(&mut app);
    app.selection.active_card_id = Some(target_id);

    app.handle_manage_children();

    assert!(
        app.relationship.card_ids.contains(&live_id),
        "live candidate must be offered"
    );
    assert!(
        !app.relationship.card_ids.contains(&archived_id),
        "archived candidate must be excluded when the target is live"
    );
}

#[test]
fn test_card_detail_manage_parents_excludes_archived_candidates() {
    let mut app = App::test_default();
    let (_board_id, target_id, live_id, archived_id) = seed_target_and_candidates(&mut app);
    sync_model_from_store(&mut app);
    app.selection.active_card_id = Some(target_id);

    app.handle_manage_parents();

    assert!(
        app.relationship.card_ids.contains(&live_id),
        "live candidate must be offered"
    );
    assert!(
        !app.relationship.card_ids.contains(&archived_id),
        "archived candidate must be excluded when the target is live"
    );
}
