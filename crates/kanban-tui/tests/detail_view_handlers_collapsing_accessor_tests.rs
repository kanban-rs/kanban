use crossterm::event::KeyCode;
use kanban_domain::resolved::Collection;
use kanban_domain::{
    Board, Card, Column, KanbanOperations, LoadState, Model, ModelLoadStates, Resolved, Sprint,
};
use kanban_tui::app::{AppMode, DialogMode};
use kanban_tui::App;
use std::collections::HashMap;
use uuid::Uuid;

fn set_model(app: &mut App, states: ModelLoadStates) {
    app.model = Model::with_load_states(states);
}

fn set_sprint_by_id(app: &mut App, sprint_id: Uuid, state: LoadState<Sprint>) {
    let _ = app.model.apply_resolved(Resolved {
        sprints: Collection {
            by_id: HashMap::from([(sprint_id, state)]),
            ..Default::default()
        },
        ..Default::default()
    });
}

fn set_column_by_id(app: &mut App, column_id: Uuid, state: LoadState<Column>) {
    let _ = app.model.apply_resolved(Resolved {
        columns: Collection {
            by_id: HashMap::from([(column_id, state)]),
            ..Default::default()
        },
        ..Default::default()
    });
}

fn assert_error_banner(app: &App) {
    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("expected an error banner to be set");
    assert_eq!(banner.variant, kanban_tui::components::BannerVariant::Error);
}

fn assert_no_banner(app: &App) {
    assert!(
        app.ui_state.banner.is_none(),
        "expected no banner, got {:?}",
        app.ui_state.banner
    );
}

#[test]
fn test_carry_over_active_sprint_if_eligible_declines_when_sprint_tier_not_loaded() {
    let mut app = App::test_default();
    let sprint_id = Uuid::new_v4();
    app.selection.active_sprint_id = Some(sprint_id);

    app.handle_sprint_detail_key(KeyCode::Char('M'));

    assert_error_banner(&app);
}

#[test]
fn test_carry_over_active_sprint_if_eligible_distinguishes_missing_from_not_loaded() {
    let mut app = App::test_default();
    let missing_sprint_id = Uuid::new_v4();
    app.selection.active_sprint_id = Some(missing_sprint_id);
    set_sprint_by_id(&mut app, missing_sprint_id, LoadState::Missing);

    app.handle_sprint_detail_key(KeyCode::Char('M'));
    assert_no_banner(&app);

    let not_loaded_sprint_id = Uuid::new_v4();
    app.selection.active_sprint_id = Some(not_loaded_sprint_id);

    app.handle_sprint_detail_key(KeyCode::Char('M'));
    assert_error_banner(&app);
}

#[test]
fn test_open_assign_sprint_dialog_for_declines_when_sprints_not_loaded() {
    let mut app = App::test_default();
    let board = Board::new("Board", None::<String>);
    let board_id = board.id;
    let column = Column::new(board_id, "Todo", 0);
    let card = Card::new(board_id, column.id, "Card", 0);
    let card_id = card.id;
    set_model(
        &mut app,
        ModelLoadStates {
            boards: LoadState::Loaded(vec![board]),
            cards: LoadState::Loaded(vec![card]),
            ..Default::default()
        },
    );
    app.selection.active_board_id = Some(board_id);
    app.sprint_view.panel = kanban_tui::app::SprintTaskPanel::Uncompleted;
    app.sprint_view
        .uncompleted_component
        .update_cards(vec![card_id]);
    app.sprint_view
        .uncompleted_component
        .set_selected_index(Some(0));

    app.handle_sprint_detail_key(KeyCode::Char('s'));

    assert!(!matches!(
        app.mode,
        AppMode::Dialog(DialogMode::AssignCardToSprint)
    ));
    assert_error_banner(&app);
}

#[test]
fn test_open_assign_sprint_dialog_for_still_opens_when_sprints_loaded() {
    let mut app = App::test_default();
    let board = Board::new("Board", None::<String>);
    let board_id = board.id;
    let column = Column::new(board_id, "Todo", 0);
    let card = Card::new(board_id, column.id, "Card", 0);
    let card_id = card.id;
    let sprint = Sprint::new(board_id, 1, None, None::<String>);
    set_model(
        &mut app,
        ModelLoadStates {
            boards: LoadState::Loaded(vec![board]),
            cards: LoadState::Loaded(vec![card]),
            sprints: LoadState::Loaded(vec![sprint.clone()]),
            ..Default::default()
        },
    );
    app.selection.active_board_id = Some(board_id);
    app.sprint_view.panel = kanban_tui::app::SprintTaskPanel::Uncompleted;
    app.sprint_view
        .uncompleted_component
        .update_cards(vec![card_id]);
    app.sprint_view
        .uncompleted_component
        .set_selected_index(Some(0));

    app.handle_sprint_detail_key(KeyCode::Char('s'));

    assert!(matches!(
        app.mode,
        AppMode::Dialog(DialogMode::AssignCardToSprint)
    ));
    assert_no_banner(&app);
}

#[test]
fn test_handle_sprint_detail_key_declines_prefix_edit_when_sprint_tier_not_loaded() {
    let mut app = App::test_default();
    let sprint_id = Uuid::new_v4();
    app.selection.active_sprint_id = Some(sprint_id);

    app.handle_sprint_detail_key(KeyCode::Char('p'));

    assert!(!matches!(
        app.mode,
        AppMode::Dialog(DialogMode::SetSprintPrefix)
    ));
    assert_error_banner(&app);
}

#[test]
fn test_handle_sprint_detail_key_still_opens_prefix_edit_when_sprint_tier_loaded() {
    let mut app = App::test_default();
    let board = Board::new("Board", None::<String>);
    let sprint = Sprint::new(board.id, 1, None, Some("PRE".to_string()));
    let sprint_id = sprint.id;
    set_sprint_by_id(&mut app, sprint_id, LoadState::Loaded(sprint));
    app.selection.active_sprint_id = Some(sprint_id);

    app.handle_sprint_detail_key(KeyCode::Char('p'));

    assert!(matches!(
        app.mode,
        AppMode::Dialog(DialogMode::SetSprintPrefix)
    ));
    assert_no_banner(&app);
    assert_eq!(app.input.as_str(), "PRE");
}

#[test]
fn test_handle_manage_parents_declines_when_the_cards_column_is_not_loaded() {
    let mut app = App::test_default();
    let board = Board::new("Board", None::<String>);
    let column = Column::new(board.id, "Todo", 0);
    let card = Card::new(board.id, column.id, "Card", 0);
    let card_id = card.id;
    set_model(
        &mut app,
        ModelLoadStates {
            cards: LoadState::Loaded(vec![card]),
            ..Default::default()
        },
    );
    app.selection.active_card_id = Some(card_id);

    app.handle_manage_parents();

    assert!(!matches!(
        app.mode,
        AppMode::Dialog(DialogMode::ManageParents)
    ));
    assert_error_banner(&app);
}

#[test]
fn test_handle_manage_parents_declines_when_the_boards_column_list_is_not_loaded() {
    let mut app = App::test_default();
    let board = Board::new("Board", None::<String>);
    let column = Column::new(board.id, "Todo", 0);
    let column_id = column.id;
    let card = Card::new(board.id, column.id, "Card", 0);
    let card_id = card.id;
    set_model(
        &mut app,
        ModelLoadStates {
            cards: LoadState::Loaded(vec![card]),
            ..Default::default()
        },
    );
    set_column_by_id(&mut app, column_id, LoadState::Loaded(column));
    app.selection.active_card_id = Some(card_id);

    app.handle_manage_parents();

    assert!(!matches!(
        app.mode,
        AppMode::Dialog(DialogMode::ManageParents)
    ));
    assert_error_banner(&app);
}

#[test]
fn test_handle_manage_parents_still_opens_the_dialog_when_everything_is_loaded() {
    let mut app = App::test_default();
    let board = Board::new("Board", None::<String>);
    let board_id = board.id;
    let column = Column::new(board_id, "Todo", 0);
    let column_id = column.id;
    let card = Card::new(board_id, column_id, "Card", 0);
    let card_id = card.id;
    let other_card = Card::new(board_id, column_id, "Other", 1);
    set_model(
        &mut app,
        ModelLoadStates {
            cards: LoadState::Loaded(vec![card, other_card]),
            columns: LoadState::Loaded(vec![column.clone()]),
            graph: LoadState::Loaded(kanban_domain::DependencyGraph::default()),
            ..Default::default()
        },
    );
    set_column_by_id(&mut app, column_id, LoadState::Loaded(column));
    app.selection.active_card_id = Some(card_id);

    app.handle_manage_parents();

    assert!(matches!(
        app.mode,
        AppMode::Dialog(DialogMode::ManageParents)
    ));
    assert_no_banner(&app);
    assert_eq!(app.relationship.card_ids.len(), 1);
}

#[test]
fn test_handle_manage_children_declines_when_the_cards_column_is_not_loaded() {
    let mut app = App::test_default();
    let board = Board::new("Board", None::<String>);
    let column = Column::new(board.id, "Todo", 0);
    let card = Card::new(board.id, column.id, "Card", 0);
    let card_id = card.id;
    set_model(
        &mut app,
        ModelLoadStates {
            cards: LoadState::Loaded(vec![card]),
            ..Default::default()
        },
    );
    app.selection.active_card_id = Some(card_id);

    app.handle_manage_children();

    assert!(!matches!(
        app.mode,
        AppMode::Dialog(DialogMode::ManageChildren)
    ));
    assert_error_banner(&app);
}

#[test]
fn test_handle_manage_children_declines_when_the_boards_column_list_is_not_loaded() {
    let mut app = App::test_default();
    let board = Board::new("Board", None::<String>);
    let column = Column::new(board.id, "Todo", 0);
    let column_id = column.id;
    let card = Card::new(board.id, column.id, "Card", 0);
    let card_id = card.id;
    set_model(
        &mut app,
        ModelLoadStates {
            cards: LoadState::Loaded(vec![card]),
            ..Default::default()
        },
    );
    set_column_by_id(&mut app, column_id, LoadState::Loaded(column));
    app.selection.active_card_id = Some(card_id);

    app.handle_manage_children();

    assert!(!matches!(
        app.mode,
        AppMode::Dialog(DialogMode::ManageChildren)
    ));
    assert_error_banner(&app);
}

#[test]
fn test_handle_manage_children_still_opens_the_dialog_when_everything_is_loaded() {
    let mut app = App::test_default();
    let board = Board::new("Board", None::<String>);
    let board_id = board.id;
    let column = Column::new(board_id, "Todo", 0);
    let column_id = column.id;
    let card = Card::new(board_id, column_id, "Card", 0);
    let card_id = card.id;
    let other_card = Card::new(board_id, column_id, "Other", 1);
    set_model(
        &mut app,
        ModelLoadStates {
            cards: LoadState::Loaded(vec![card, other_card]),
            columns: LoadState::Loaded(vec![column.clone()]),
            graph: LoadState::Loaded(kanban_domain::DependencyGraph::default()),
            ..Default::default()
        },
    );
    set_column_by_id(&mut app, column_id, LoadState::Loaded(column));
    app.selection.active_card_id = Some(card_id);

    app.handle_manage_children();

    assert!(matches!(
        app.mode,
        AppMode::Dialog(DialogMode::ManageChildren)
    ));
    assert_no_banner(&app);
    assert_eq!(app.relationship.card_ids.len(), 1);
}

fn seed_move_fixture(app: &mut App, columns_loaded: bool) -> (Uuid, Uuid, Uuid, Uuid) {
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let left = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    let right = app
        .ctx
        .create_column(board.id, "Doing".to_string(), Some(1))
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            left.id,
            "Card".to_string(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let columns = if columns_loaded {
        LoadState::Loaded(vec![left.clone(), right.clone()])
    } else {
        LoadState::NotLoaded
    };
    set_model(
        app,
        ModelLoadStates {
            boards: LoadState::Loaded(vec![board.clone()]),
            cards: LoadState::Loaded(vec![card.clone()]),
            columns,
            ..Default::default()
        },
    );
    app.selection.active_board_id = Some(board.id);
    app.sprint_view.panel = kanban_tui::app::SprintTaskPanel::Uncompleted;
    app.sprint_view
        .uncompleted_component
        .update_cards(vec![card.id]);
    app.sprint_view
        .uncompleted_component
        .set_selected_index(Some(0));
    (board.id, left.id, right.id, card.id)
}

#[test]
fn test_move_selected_card_column_declines_when_the_column_tier_is_not_loaded() {
    let mut app = App::test_default();
    let (_board_id, left_id, _right_id, card_id) = seed_move_fixture(&mut app, false);

    app.handle_sprint_detail_key(KeyCode::Char('L'));

    assert_error_banner(&app);
    let stored_column = app
        .model
        .cards_state()
        .loaded_or_empty()
        .iter()
        .find(|c| c.id == card_id)
        .map(|c| c.column_id)
        .expect("card still present");
    assert_eq!(stored_column, left_id);
}

#[test]
fn test_move_selected_card_column_still_moves_when_the_column_tier_is_loaded() {
    let mut app = App::test_default();
    let (_board_id, _left_id, right_id, card_id) = seed_move_fixture(&mut app, true);

    app.handle_sprint_detail_key(KeyCode::Char('L'));

    assert_no_banner(&app);
    app.reload_model();
    let moved_column = app
        .model
        .cards_state()
        .loaded_or_empty()
        .iter()
        .find(|c| c.id == card_id)
        .map(|c| c.column_id)
        .expect("card still present");
    assert_eq!(moved_column, right_id);
}
