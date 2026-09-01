//! `card_handlers.rs` reads several `Model::columns()`/`Model::sprints()`
//! call sites that collapse a `NotLoaded` tier to an empty slice, so a stale
//! read silently behaves as "board has zero columns/sprints" instead of
//! declining. These tests pin the decline behaviour.

use kanban_domain::resolved::Collection;
use kanban_domain::{
    ArchivedCard, CreateCardOptions, DerivedProjections, EntityIds, Invalidation, KanbanOperations,
    LoadState, NoProjections, Resolved,
};
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::app::Focus;
use kanban_tui::App;
use kanban_view::card_list::CardListId;
use uuid::Uuid;

fn sync_model_from_store(app: &mut App) {
    let snapshot = app.ctx.snapshot().unwrap();
    app.load_snapshot(snapshot);
}

fn seed_board_with_two_columns(app: &mut App) -> (Uuid, Uuid, Uuid) {
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let first = app
        .ctx
        .create_column(board.id, "Todo".into(), Some(0))
        .unwrap();
    let second = app
        .ctx
        .create_column(board.id, "Doing".into(), Some(1))
        .unwrap();
    (board.id, first.id, second.id)
}

fn select_card_in_active_task_list(app: &mut App, card_id: Uuid) {
    app.prepare_frame();
    let list = app
        .view
        .strategy
        .get_active_task_list_mut()
        .expect("active task list");
    let idx = list
        .cards
        .iter()
        .position(|&id| id == card_id)
        .expect("card present in active task list");
    list.set_selected_index(Some(idx));
}

#[test]
fn test_handle_create_card_key_with_a_not_loaded_sprint_tier_declines() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    app.ctx
        .create_column(board.id, "Todo".into(), Some(0))
        .unwrap();
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Cards;

    let _ = app
        .model
        .invalidate(Invalidation::Entities(EntityIds::sprints([Uuid::new_v4()])));

    app.handle_create_card_key();

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("declining a NotLoaded sprints tier must set an error banner");
    assert!(
        banner.message.to_lowercase().contains("sprint"),
        "banner message must name sprints as not loaded, got: {}",
        banner.message
    );
    assert!(
        !matches!(app.mode, AppMode::Dialog(DialogMode::CreateCard)),
        "the create-card dialog must not open on a declined sprints tier"
    );
}

#[test]
fn test_handle_create_card_key_still_opens_on_a_loaded_sprint_tier() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    app.ctx
        .create_column(board.id, "Todo".into(), Some(0))
        .unwrap();
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Cards;

    app.handle_create_card_key();

    assert!(matches!(app.mode, AppMode::Dialog(DialogMode::CreateCard)));
    assert!(app.ui_state.banner.is_none());
}

#[test]
fn test_create_card_target_column_prefers_board_scoped_state_over_stale_flat_tier() {
    let mut app = App::test_default();
    let (board_id, first_col, _second_col) = seed_board_with_two_columns(&mut app);
    sync_model_from_store(&mut app);
    let column = app
        .model
        .column_by_id_state(first_col)
        .loaded()
        .copied()
        .cloned()
        .unwrap();

    let _ = app
        .model
        .invalidate(Invalidation::Entities(EntityIds::columns([Uuid::new_v4()])));
    assert!(app.model.columns_state().is_not_loaded());

    let changed = app.model.apply_resolved(Resolved {
        columns: Collection {
            by_parent: [(board_id, LoadState::Loaded(vec![column]))].into(),
            ..Default::default()
        },
        ..Default::default()
    });
    NoProjections.resync(&app.model, changed);

    let target = app.create_card_target_column(board_id);

    assert_eq!(
        target.map(|c| c.id),
        Some(first_col),
        "must be served from the board-scoped tier even though the flat tier is stale"
    );
}

#[test]
fn test_create_card_target_column_returns_none_when_the_scoped_tier_is_genuinely_empty() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    sync_model_from_store(&mut app);

    let changed = app.model.apply_resolved(Resolved {
        columns: Collection {
            by_parent: [(board.id, LoadState::Loaded(vec![]))].into(),
            ..Default::default()
        },
        ..Default::default()
    });
    NoProjections.resync(&app.model, changed);

    let target = app.create_card_target_column(board.id);

    assert!(target.is_none());
}

#[test]
fn test_create_card_target_column_distinguishes_missing_from_not_loaded() {
    let mut app = App::test_default();
    let (_board_id, first_col, _second_col) = seed_board_with_two_columns(&mut app);
    sync_model_from_store(&mut app);

    let missing_id = Uuid::new_v4();
    let changed = app.model.apply_resolved(Resolved {
        columns: Collection {
            by_id: [(missing_id, LoadState::Missing)].into(),
            ..Default::default()
        },
        ..Default::default()
    });
    NoProjections.resync(&app.model, changed);
    assert!(matches!(
        app.model.column_by_id_state(missing_id),
        LoadState::Missing
    ));

    let _ = app
        .model
        .invalidate(Invalidation::Entities(EntityIds::columns([first_col])));
    assert!(matches!(
        app.model.column_by_id_state(first_col),
        LoadState::NotLoaded
    ));

    assert_eq!(
        app.model
            .column_by_id_state(missing_id)
            .loaded()
            .copied()
            .cloned(),
        None,
        "a genuinely missing id resolves to None, the fabricate-a-column arm's input"
    );
    assert_eq!(
        app.model
            .column_by_id_state(first_col)
            .loaded()
            .copied()
            .cloned(),
        None,
        "a stale not-loaded id collapses to the same None as missing at this shape-b site"
    );
}

fn seed_board_column_sprint_card(app: &mut App) -> (Uuid, Uuid, Uuid, Uuid) {
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".into(), Some(0))
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
    (board.id, column.id, sprint.id, card.id)
}

#[test]
fn test_handle_assign_to_sprint_key_declines_on_a_not_loaded_sprint_tier_bulk_path() {
    let mut app = App::test_default();
    let (board_id, _column_id, _sprint_id, card_id) = seed_board_column_sprint_card(&mut app);
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.focus.active = Focus::Cards;
    app.multi_select.selected_cards.insert(card_id);

    let _ = app
        .model
        .invalidate(Invalidation::Entities(EntityIds::sprints([Uuid::new_v4()])));

    app.handle_assign_to_sprint_key();

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("declining a NotLoaded sprints tier must set an error banner");
    assert!(banner.message.to_lowercase().contains("sprint"));
    assert!(!matches!(
        app.mode,
        AppMode::Dialog(DialogMode::AssignMultipleCardsToSprint)
    ));
}

#[test]
fn test_handle_assign_to_sprint_key_still_opens_bulk_dialog_on_a_loaded_sprint_tier() {
    let mut app = App::test_default();
    let (board_id, _column_id, _sprint_id, card_id) = seed_board_column_sprint_card(&mut app);
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.focus.active = Focus::Cards;
    app.multi_select.selected_cards.insert(card_id);

    app.handle_assign_to_sprint_key();

    assert!(matches!(
        app.mode,
        AppMode::Dialog(DialogMode::AssignMultipleCardsToSprint)
    ));
    assert!(app.ui_state.banner.is_none());
}

#[test]
fn test_handle_assign_to_sprint_key_declines_on_a_not_loaded_sprint_tier_single_card_path() {
    let mut app = App::test_default();
    let (board_id, _column_id, _sprint_id, card_id) = seed_board_column_sprint_card(&mut app);
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.focus.active = Focus::Cards;
    select_card_in_active_task_list(&mut app, card_id);

    let _ = app
        .model
        .invalidate(Invalidation::Entities(EntityIds::sprints([Uuid::new_v4()])));

    app.handle_assign_to_sprint_key();

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("declining a NotLoaded sprints tier must set an error banner");
    assert!(banner.message.to_lowercase().contains("sprint"));
    assert!(!matches!(
        app.mode,
        AppMode::Dialog(DialogMode::AssignCardToSprint)
    ));
}

#[test]
fn test_handle_assign_to_sprint_key_still_opens_single_card_dialog_on_a_loaded_sprint_tier() {
    let mut app = App::test_default();
    let (board_id, _column_id, _sprint_id, card_id) = seed_board_column_sprint_card(&mut app);
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.focus.active = Focus::Cards;
    select_card_in_active_task_list(&mut app, card_id);

    app.handle_assign_to_sprint_key();

    assert!(matches!(
        app.mode,
        AppMode::Dialog(DialogMode::AssignCardToSprint)
    ));
    assert!(app.ui_state.banner.is_none());
}

#[test]
fn test_create_card_declines_on_a_not_loaded_column_tier() {
    let mut app = App::test_default();
    let (board_id, _first_col, _second_col) = seed_board_with_two_columns(&mut app);
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);

    let _ = app
        .model
        .invalidate(Invalidation::Entities(EntityIds::columns([Uuid::new_v4()])));

    let columns_before = app.ctx.data_store().list_all_columns().unwrap().len();

    app.input.set("Should not be created".to_string());
    app.create_card();
    app.input.clear();

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("declining a NotLoaded columns tier must set an error banner");
    assert!(banner.message.to_lowercase().contains("column"));

    let cards = app.ctx.data_store().list_all_cards().unwrap();
    assert!(
        cards.is_empty(),
        "no card must be created while the columns tier is declined"
    );
    let columns_after = app.ctx.data_store().list_all_columns().unwrap().len();
    assert_eq!(
        columns_before, columns_after,
        "no fabricated column must be created while the columns tier is declined"
    );
}

#[test]
fn test_create_card_declines_on_a_not_loaded_column_tier_with_a_column_focused() {
    let mut app = App::test_default();
    let (board_id, first_col, _second_col) = seed_board_with_two_columns(&mut app);
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.switch_view_strategy(kanban_domain::TaskListView::ColumnView);
    app.prepare_frame();

    let focused = app
        .view
        .strategy
        .get_active_task_list()
        .map(|list| list.id.clone());
    assert!(
        matches!(focused, Some(CardListId::Column(id)) if id == first_col),
        "expected the active task list to be the focused column, got {focused:?}"
    );

    let _ = app
        .model
        .invalidate(Invalidation::Entities(EntityIds::columns([Uuid::new_v4()])));

    let columns_before = app.ctx.data_store().list_all_columns().unwrap().len();

    app.input.set("Should not be created".to_string());
    app.create_card();
    app.input.clear();

    let banner = app.ui_state.banner.as_ref().expect(
        "declining a NotLoaded columns tier must set an error banner even with a column focused",
    );
    assert!(banner.message.to_lowercase().contains("column"));

    let cards = app.ctx.data_store().list_all_cards().unwrap();
    assert!(
        cards.is_empty(),
        "no card must be created while the columns tier is declined"
    );
    let columns_after = app.ctx.data_store().list_all_columns().unwrap().len();
    assert_eq!(
        columns_before, columns_after,
        "no fabricated column must be created while the columns tier is declined, even with a column focused"
    );
}

#[test]
fn test_create_card_still_auto_completes_into_a_completion_column_on_a_loaded_column_tier() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Done".into(), Some(0))
        .unwrap();
    app.ctx
        .update_column(
            column.id,
            kanban_domain::ColumnUpdate {
                default_status: Some(Some(kanban_domain::CardStatus::Done)),
                ..Default::default()
            },
        )
        .unwrap();
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board.id);

    let columns = app
        .model
        .columns_state()
        .loaded()
        .cloned()
        .unwrap_or_default();
    let changed = app.model.apply_resolved(Resolved {
        columns: Collection {
            by_parent: [(board.id, LoadState::Loaded(columns))].into(),
            ..Default::default()
        },
        ..Default::default()
    });
    NoProjections.resync(&app.model, changed);

    app.input.set("Auto-complete".to_string());
    app.create_card();
    app.input.clear();

    let cards = app.ctx.data_store().list_all_cards().unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].status, kanban_domain::CardStatus::Done);
    assert!(app.ui_state.banner.is_none());
}

#[test]
fn test_handle_move_card_declines_on_a_not_loaded_column_tier() {
    let mut app = App::test_default();
    let (board_id, first_col, _second_col) = seed_board_with_two_columns(&mut app);
    let card = app
        .ctx
        .create_card(
            board_id,
            first_col,
            "Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.focus.active = Focus::Cards;
    select_card_in_active_task_list(&mut app, card.id);

    let _ = app
        .model
        .invalidate(Invalidation::Entities(EntityIds::columns([Uuid::new_v4()])));

    app.handle_move_card_right();

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("declining a NotLoaded columns tier must set an error banner");
    assert!(banner.message.to_lowercase().contains("column"));

    let stored = app.ctx.data_store().get_card(card.id).unwrap().unwrap();
    assert_eq!(
        stored.column_id, first_col,
        "the card must not move while the columns tier is declined"
    );
}

#[test]
fn test_handle_move_card_still_moves_the_card_on_a_loaded_column_tier() {
    let mut app = App::test_default();
    let (board_id, first_col, second_col) = seed_board_with_two_columns(&mut app);
    let card = app
        .ctx
        .create_card(
            board_id,
            first_col,
            "Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.focus.active = Focus::Cards;
    select_card_in_active_task_list(&mut app, card.id);

    app.handle_move_card_right();

    let stored = app.ctx.data_store().get_card(card.id).unwrap().unwrap();
    assert_eq!(stored.column_id, second_col);
}

#[test]
fn test_move_selected_cards_declines_on_a_not_loaded_column_tier() {
    let mut app = App::test_default();
    let (board_id, first_col, _second_col) = seed_board_with_two_columns(&mut app);
    let card_a = app
        .ctx
        .create_card(
            board_id,
            first_col,
            "A".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let card_b = app
        .ctx
        .create_card(
            board_id,
            first_col,
            "B".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.focus.active = Focus::Cards;
    app.multi_select.selected_cards.insert(card_a.id);
    app.multi_select.selected_cards.insert(card_b.id);

    let _ = app
        .model
        .invalidate(Invalidation::Entities(EntityIds::columns([Uuid::new_v4()])));

    app.handle_move_card_right();

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("declining a NotLoaded columns tier must set an error banner");
    assert!(banner.message.to_lowercase().contains("column"));

    let stored_a = app.ctx.data_store().get_card(card_a.id).unwrap().unwrap();
    let stored_b = app.ctx.data_store().get_card(card_b.id).unwrap().unwrap();
    assert_eq!(stored_a.column_id, first_col);
    assert_eq!(stored_b.column_id, first_col);
}

#[test]
fn test_restore_card_without_reload_declines_on_a_not_loaded_column_tier() {
    let mut app = App::test_default();
    let (board_id, first_col, _second_col) = seed_board_with_two_columns(&mut app);
    let card = app
        .ctx
        .create_card(
            board_id,
            first_col,
            "Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.archive_card(card.id).unwrap();
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);

    let _ = app
        .model
        .invalidate(Invalidation::Entities(EntityIds::columns([Uuid::new_v4()])));

    let archived = ArchivedCard::new(card.id, board_id);
    app.restore_card(archived);

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("declining a NotLoaded columns tier must set an error banner");
    assert!(banner.message.to_lowercase().contains("column"));
    let still_archived = app
        .ctx
        .data_store()
        .list_archived_cards()
        .unwrap()
        .iter()
        .any(|a| a.entity_id == card.id);
    assert!(
        still_archived,
        "the card must remain archived while the columns tier is declined"
    );
}

#[test]
fn test_restore_card_without_reload_remaps_to_first_column_when_the_original_is_gone() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let original = app
        .ctx
        .create_column(board.id, "Todo".into(), Some(0))
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            original.id,
            "Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.archive_card(card.id).unwrap();
    app.ctx.delete_column(original.id).unwrap();
    let replacement = app
        .ctx
        .create_column(board.id, "Backlog".into(), Some(0))
        .unwrap();
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board.id);

    let archived = ArchivedCard::new(card.id, board.id);
    app.restore_card(archived);

    let stored = app.ctx.data_store().get_card(card.id).unwrap().unwrap();
    assert_eq!(stored.column_id, replacement.id);
    assert!(app.ui_state.banner.is_none());
}

// ---------------------------------------------------------------------
// handle_manage_children_from_list: decline on a NotLoaded board-scoped
// columns tier
// ---------------------------------------------------------------------

#[test]
fn test_handle_manage_children_from_list_declines_on_a_not_loaded_column_tier() {
    let mut app = App::test_default();
    let (board_id, first_col, _second_col) = seed_board_with_two_columns(&mut app);
    let card = app
        .ctx
        .create_card(
            board_id,
            first_col,
            "Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.focus.active = Focus::Cards;
    select_card_in_active_task_list(&mut app, card.id);
    app.relationship.card_ids.clear();

    assert!(
        app.model.board_columns_state(board_id).is_not_loaded(),
        "reload never populates the board-scoped columns tier today"
    );
    let _ = app
        .model
        .invalidate(Invalidation::Entities(EntityIds::columns([Uuid::new_v4()])));

    app.handle_manage_children_from_list();

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("declining a NotLoaded board-scoped columns tier must set an error banner");
    assert!(banner.message.to_lowercase().contains("column"));
    assert!(
        app.relationship.card_ids.is_empty(),
        "the manage-children list must not be populated while the columns tier is declined"
    );
}

#[test]
fn test_handle_manage_children_from_list_still_lists_a_sibling_card_on_a_loaded_column_tier() {
    let mut app = App::test_default();
    let (board_id, first_col, _second_col) = seed_board_with_two_columns(&mut app);
    let target = app
        .ctx
        .create_card(
            board_id,
            first_col,
            "Target".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let sibling = app
        .ctx
        .create_card(
            board_id,
            first_col,
            "Sibling".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    sync_model_from_store(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.focus.active = Focus::Cards;
    select_card_in_active_task_list(&mut app, target.id);

    let columns = app
        .model
        .columns_state()
        .loaded()
        .cloned()
        .unwrap_or_default();
    let changed = app.model.apply_resolved(Resolved {
        columns: Collection {
            by_parent: [(board_id, LoadState::Loaded(columns))].into(),
            ..Default::default()
        },
        ..Default::default()
    });
    NoProjections.resync(&app.model, changed);

    app.handle_manage_children_from_list();

    assert!(app.ui_state.banner.is_none());
    assert!(
        app.relationship.card_ids.contains(&sibling.id),
        "a sibling card in-column must still be listed as eligible"
    );
}
