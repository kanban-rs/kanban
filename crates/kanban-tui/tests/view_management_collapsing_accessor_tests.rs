//! `App::prepare_frame` reads `Model::columns()`/`Model::sprints()`, which
//! collapse a `NotLoaded` tier to an empty slice, so a stale read silently
//! rebuilds the task lists as if the board had zero columns/sprints instead
//! of declining. These tests pin the decline behaviour.

use kanban_domain::{CreateCardOptions, EntityIds, Invalidation, KanbanOperations};
use kanban_tui::App;
use uuid::Uuid;

fn sync_model_from_store(app: &mut App) {
    let snapshot = app.ctx.snapshot().unwrap();
    app.load_snapshot(snapshot);
}

fn seed_board_with_column_and_card(app: &mut App) -> (Uuid, Uuid) {
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".into(), Some(0))
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    (board.id, card.id)
}

#[test]
fn test_prepare_frame_leaves_the_task_lists_intact_when_the_tiers_are_unloaded() {
    let mut app = App::test_default();
    let (board_id, card_id) = seed_board_with_column_and_card(&mut app);
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);

    app.prepare_frame();
    let baseline = app
        .view
        .strategy
        .get_active_task_list()
        .expect("active task list")
        .cards
        .clone();
    assert_eq!(
        baseline,
        vec![card_id],
        "fixture sanity: the seeded card must appear in the task list while loaded"
    );

    let mut ids = EntityIds::columns([Uuid::new_v4()]);
    ids.merge(EntityIds::sprints([Uuid::new_v4()]));
    let _ = app.model.invalidate(Invalidation::Entities(ids));

    app.prepare_frame();

    let after = app
        .view
        .strategy
        .get_active_task_list()
        .expect("active task list")
        .cards
        .clone();
    assert_eq!(
        after, baseline,
        "an unloaded columns/sprints tier must decline, leaving the prior task lists untouched"
    );
}

#[test]
fn test_prepare_frame_rebuilds_the_task_lists_when_the_tiers_are_loaded() {
    let mut app = App::test_default();
    let (board_id, card_id) = seed_board_with_column_and_card(&mut app);
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);

    app.prepare_frame();

    let list = app
        .view
        .strategy
        .get_active_task_list()
        .expect("active task list");
    assert_eq!(list.cards, vec![card_id]);
}
