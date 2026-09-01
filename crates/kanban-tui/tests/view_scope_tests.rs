use kanban_domain::{CreateCardOptions, KanbanOperations};
use kanban_tui::app::mode::{AppMode, DialogMode};
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
fn test_normal_mode_scopes_the_highlighted_board_when_none_is_opened() {
    let mut app = App::test_default();
    let (board_id, ..) = seed_board_column_sprint_card(&mut app);
    app.reload_model();
    app.board_list.update_boards(vec![board_id]);

    let scope = app.view_scope();

    assert_eq!(scope.board, Some(board_id));
}

#[test]
fn test_card_detail_scopes_the_active_card_the_sprints_and_the_graph() {
    let mut app = App::test_default();
    let (board_id, _column_id, _sprint_id, card_id) = seed_board_column_sprint_card(&mut app);
    app.reload_model();
    app.selection.active_board_id = Some(board_id);
    app.selection.active_card_id = Some(card_id);
    app.mode = AppMode::CardDetail;

    let scope = app.view_scope();

    assert_eq!(scope.card, Some(card_id));
    assert!(scope.board_sprints);
    assert!(scope.graph);
}

#[test]
fn test_sprint_detail_scopes_the_active_sprint() {
    let mut app = App::test_default();
    let (board_id, _column_id, sprint_id, _card_id) = seed_board_column_sprint_card(&mut app);
    app.reload_model();
    app.selection.active_board_id = Some(board_id);
    app.selection.active_sprint_id = Some(sprint_id);
    app.mode = AppMode::SprintDetail;

    let scope = app.view_scope();

    assert_eq!(scope.sprint, Some(sprint_id));
    assert!(scope.board_sprints);
}

#[test]
fn test_archived_cards_view_keeps_the_default_board_scope_and_requests_no_archived_tier() {
    let mut app = App::test_default();
    let (board_id, ..) = seed_board_column_sprint_card(&mut app);
    app.reload_model();
    app.selection.active_board_id = Some(board_id);
    app.mode = AppMode::ArchivedCardsView;

    let scope = app.view_scope();

    assert!(scope.board_columns);
    assert!(scope.board_cards);
}

#[test]
fn test_help_over_card_detail_keeps_the_card_scope() {
    let mut app = App::test_default();
    let (board_id, _column_id, _sprint_id, card_id) = seed_board_column_sprint_card(&mut app);
    app.reload_model();
    app.selection.active_board_id = Some(board_id);
    app.selection.active_card_id = Some(card_id);
    app.mode = AppMode::Help(Box::new(AppMode::CardDetail));

    let scope = app.view_scope();

    assert_eq!(scope.card, Some(card_id));
    assert!(scope.graph);
    assert!(scope.board_sprints);
}

#[test]
fn test_manage_children_dialog_scopes_the_graph() {
    let mut app = App::test_default();
    let (board_id, ..) = seed_board_column_sprint_card(&mut app);
    app.reload_model();
    app.selection.active_board_id = Some(board_id);
    app.mode = AppMode::Normal;
    app.push_mode(AppMode::Dialog(DialogMode::ManageChildren));

    let scope = app.view_scope();

    assert!(scope.graph);
}

#[test]
fn test_an_active_sprint_filter_scopes_the_sprints_in_normal_mode() {
    let mut app = App::test_default();
    let (board_id, ..) = seed_board_column_sprint_card(&mut app);
    app.reload_model();
    app.selection.active_board_id = Some(board_id);
    app.mode = AppMode::Normal;
    app.filter.active_sprint_filters.insert(Uuid::new_v4());

    let scope = app.view_scope();

    assert!(scope.board_sprints);
}

#[test]
fn test_settings_mode_keeps_the_board_list_and_drops_the_board_subtree() {
    let mut app = App::test_default();
    let (board_id, ..) = seed_board_column_sprint_card(&mut app);
    app.reload_model();
    app.selection.active_board_id = Some(board_id);
    app.mode = AppMode::Settings;

    let scope = app.view_scope();

    assert!(scope.board_list);
    assert!(!scope.board_columns);
    assert!(!scope.board_cards);
}

#[test]
fn test_error_log_mode_keeps_the_board_columns_and_cards_it_renders_over() {
    let mut app = App::test_default();
    let (board_id, ..) = seed_board_column_sprint_card(&mut app);
    app.reload_model();
    app.selection.active_board_id = Some(board_id);
    app.mode = AppMode::ErrorLog;

    let scope = app.view_scope();

    assert!(scope.board_columns);
    assert!(scope.board_cards);
    assert!(scope.board_list);
}

#[test]
fn test_search_mode_needs_no_scope_beyond_the_board_it_filters() {
    let mut app = App::test_default();
    let (board_id, ..) = seed_board_column_sprint_card(&mut app);
    app.reload_model();
    app.selection.active_board_id = Some(board_id);
    app.mode = AppMode::Search;

    let scope = app.view_scope();

    assert!(scope.board_list);
    assert!(scope.board_columns);
    assert!(scope.board_cards);
    assert_eq!(
        scope,
        kanban_tui::app::ViewScope {
            board_list: true,
            board: Some(board_id),
            board_columns: true,
            board_cards: true,
            ..Default::default()
        }
    );
}
