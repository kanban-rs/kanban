use kanban_domain::{CreateCardOptions, EntityIds, Invalidation, KanbanOperations};
use kanban_tui::App;
use uuid::Uuid;

fn seed_board_column_sprint_card(app: &mut App) -> (Uuid, Uuid, Uuid, Uuid) {
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".into(), None)
        .unwrap();
    let sprint = app.ctx.create_sprint(board.id, None, None).unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.assign_card_to_sprint(card.id, sprint.id).unwrap();
    (board.id, column.id, sprint.id, card.id)
}

#[test]
fn test_populate_sprint_task_lists_declines_when_the_column_tier_is_not_loaded() {
    let mut app = App::test_default();
    let (board_id, _column_id, sprint_id, card_id) = seed_board_column_sprint_card(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.reload_model();

    app.populate_sprint_task_lists(sprint_id);
    assert_eq!(app.sprint_view.uncompleted_cards.cards, vec![card_id]);

    let _ = app
        .model
        .invalidate(Invalidation::Entities(EntityIds::columns([Uuid::new_v4()])));

    app.populate_sprint_task_lists(sprint_id);

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("declining a NotLoaded tier must set an error banner");
    assert!(
        banner.message.to_lowercase().contains("column")
            || banner.message.to_lowercase().contains("sprint"),
        "banner message must name the not-loaded tier, got: {}",
        banner.message
    );
    assert_eq!(
        app.sprint_view.uncompleted_cards.cards,
        vec![card_id],
        "the prior baseline must be left untouched, not silently emptied"
    );
    assert!(app.sprint_view.completed_cards.cards.is_empty());
}

#[test]
fn test_populate_sprint_task_lists_still_populates_normally_on_a_loaded_tier() {
    let mut app = App::test_default();
    let (board_id, _column_id, sprint_id, card_id) = seed_board_column_sprint_card(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.reload_model();

    app.populate_sprint_task_lists(sprint_id);

    assert_eq!(app.sprint_view.uncompleted_cards.cards, vec![card_id]);
    assert!(app.ui_state.banner.is_none());
}

#[test]
fn test_copy_branch_name_declines_when_the_sprint_tier_is_not_loaded() {
    let mut app = App::test_default();
    let (board_id, _column_id, _sprint_id, card_id) = seed_board_column_sprint_card(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.reload_model();
    app.selection.active_card_id = Some(card_id);

    let _ = app
        .model
        .invalidate(Invalidation::Entities(EntityIds::sprints([Uuid::new_v4()])));

    app.copy_branch_name();

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("declining a NotLoaded sprints tier must set an error banner");
    assert!(
        banner.message.to_lowercase().contains("sprint"),
        "banner message must specifically name sprints as not loaded, got: {}",
        banner.message
    );
}

#[test]
fn test_get_current_sprint_selection_index_no_longer_reads_the_collapsing_sprints_accessor() {
    let source = include_str!("../src/app/query.rs");
    assert!(
        !source.contains("self.model.sprints()"),
        "query.rs must read sprints via the state-preserving accessor, not the collapsing Model::sprints()"
    );
}
