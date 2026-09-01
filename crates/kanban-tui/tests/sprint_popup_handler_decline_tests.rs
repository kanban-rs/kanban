use crossterm::event::KeyCode;
use kanban_domain::model::ModelLoadStates;
use kanban_domain::{Board, LoadState, Sprint};
use kanban_tui::components::banner::BannerVariant;
use kanban_tui::App;
use uuid::Uuid;

fn seed_model_with_board(
    app: &mut App,
    boards: LoadState<Vec<Board>>,
    sprints: LoadState<Vec<Sprint>>,
) -> Uuid {
    let board = Board::new("Board", None::<String>);
    let board_id = board.id;
    let boards = match boards {
        LoadState::Loaded(_) => LoadState::Loaded(vec![board]),
        other => other,
    };
    app.model = kanban_domain::Model::with_load_states(ModelLoadStates {
        boards,
        sprints,
        ..Default::default()
    });
    board_id
}

fn assert_error_banner_mentions(app: &App, needle: &str) {
    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("expected an error banner");
    assert_eq!(banner.variant, BannerVariant::Error);
    assert!(
        banner.message.to_lowercase().contains(needle),
        "banner message {:?} did not mention {:?}",
        banner.message,
        needle
    );
}

fn assert_no_banner(app: &App) {
    assert!(
        app.ui_state.banner.is_none(),
        "expected no banner, got {:?}",
        app.ui_state.banner
    );
}

#[test]
fn test_handle_activate_sprint_key_with_a_not_loaded_sprint_tier_declines() {
    let mut app = App::test_default();
    let board_id = seed_model_with_board(&mut app, LoadState::Loaded(vec![]), LoadState::NotLoaded);
    app.selection.active_board_id = Some(board_id);
    app.selection.active_sprint_id = Some(Uuid::new_v4());

    app.handle_activate_sprint_key();

    assert_error_banner_mentions(&app, "not loaded");
}

#[test]
fn test_handle_activate_sprint_key_distinguishes_missing_from_not_loaded() {
    let mut app = App::test_default();
    let board_id = seed_model_with_board(
        &mut app,
        LoadState::Loaded(vec![]),
        LoadState::Loaded(vec![]),
    );
    app.selection.active_board_id = Some(board_id);
    app.selection.active_sprint_id = Some(Uuid::new_v4());
    app.handle_activate_sprint_key();
    assert_no_banner(&app);

    let mut app = App::test_default();
    let board_id = seed_model_with_board(&mut app, LoadState::Loaded(vec![]), LoadState::NotLoaded);
    app.selection.active_board_id = Some(board_id);
    app.selection.active_sprint_id = Some(Uuid::new_v4());
    app.handle_activate_sprint_key();
    assert_error_banner_mentions(&app, "not loaded");
}

#[test]
fn test_handle_complete_sprint_key_with_a_not_loaded_sprint_tier_declines() {
    let mut app = App::test_default();
    let board_id = seed_model_with_board(&mut app, LoadState::Loaded(vec![]), LoadState::NotLoaded);
    let sprint_id = Uuid::new_v4();
    app.selection.active_board_id = Some(board_id);
    app.selection.active_sprint_id = Some(sprint_id);

    app.handle_complete_sprint_key();

    assert_error_banner_mentions(&app, "not loaded");
    assert_eq!(app.selection.active_sprint_id, Some(sprint_id));
    assert!(app.ctx.data_store().list_all_sprints().unwrap().is_empty());
}

#[test]
fn test_handle_complete_sprint_key_distinguishes_missing_from_not_loaded() {
    let mut app = App::test_default();
    let board_id = seed_model_with_board(
        &mut app,
        LoadState::Loaded(vec![]),
        LoadState::Loaded(vec![]),
    );
    app.selection.active_board_id = Some(board_id);
    app.selection.active_sprint_id = Some(Uuid::new_v4());
    app.handle_complete_sprint_key();
    assert_no_banner(&app);

    let mut app = App::test_default();
    let board_id = seed_model_with_board(&mut app, LoadState::Loaded(vec![]), LoadState::NotLoaded);
    app.selection.active_board_id = Some(board_id);
    app.selection.active_sprint_id = Some(Uuid::new_v4());
    app.handle_complete_sprint_key();
    assert_error_banner_mentions(&app, "not loaded");
}

#[test]
fn test_handle_carry_over_for_sprint_with_a_not_loaded_sprint_tier_declines() {
    let mut app = App::test_default();
    seed_model_with_board(&mut app, LoadState::NotLoaded, LoadState::NotLoaded);

    app.handle_carry_over_for_sprint(Uuid::new_v4());

    assert_error_banner_mentions(&app, "not loaded");
    assert_eq!(app.dialog_input.carry_over_source_sprint_id, None);
}

#[test]
fn test_handle_carry_over_for_sprint_distinguishes_missing_from_not_loaded() {
    let mut app = App::test_default();
    seed_model_with_board(&mut app, LoadState::NotLoaded, LoadState::Loaded(vec![]));
    app.handle_carry_over_for_sprint(Uuid::new_v4());
    assert_no_banner(&app);

    let mut app = App::test_default();
    seed_model_with_board(&mut app, LoadState::NotLoaded, LoadState::NotLoaded);
    app.handle_carry_over_for_sprint(Uuid::new_v4());
    assert_error_banner_mentions(&app, "not loaded");
}

#[test]
fn test_create_sprint_with_a_not_loaded_sprint_tier_declines() {
    let mut app = App::test_default();
    let board_id = seed_model_with_board(&mut app, LoadState::Loaded(vec![]), LoadState::NotLoaded);
    app.selection.active_board_id = Some(board_id);
    app.input.set("Alpha".to_string());

    app.create_sprint();

    assert_error_banner_mentions(&app, "not loaded");
    assert!(app.ctx.data_store().list_all_sprints().unwrap().is_empty());
}

#[test]
fn test_handle_assign_card_to_sprint_popup_with_a_not_loaded_sprint_tier_declines() {
    let mut app = App::test_default();
    let board_id = seed_model_with_board(&mut app, LoadState::Loaded(vec![]), LoadState::NotLoaded);
    app.selection.active_board_id = Some(board_id);
    let cursor_before = app.dialog_input.assign_sprint_picker.cursor_sprint_id();
    let selected_before = app.dialog_input.assign_sprint_picker.selected_sprint_id();

    app.handle_assign_card_to_sprint_popup(KeyCode::Down);

    assert_error_banner_mentions(&app, "not loaded");
    assert_eq!(
        app.dialog_input.assign_sprint_picker.cursor_sprint_id(),
        cursor_before
    );
    assert_eq!(
        app.dialog_input.assign_sprint_picker.selected_sprint_id(),
        selected_before
    );
}

#[test]
fn test_handle_assign_multiple_cards_to_sprint_popup_with_a_not_loaded_sprint_tier_declines() {
    let mut app = App::test_default();
    let board_id = seed_model_with_board(&mut app, LoadState::Loaded(vec![]), LoadState::NotLoaded);
    app.selection.active_board_id = Some(board_id);
    let cursor_before = app.dialog_input.assign_sprint_picker.cursor_sprint_id();
    let selected_before = app.dialog_input.assign_sprint_picker.selected_sprint_id();

    app.handle_assign_multiple_cards_to_sprint_popup(KeyCode::Down);

    assert_error_banner_mentions(&app, "not loaded");
    assert_eq!(
        app.dialog_input.assign_sprint_picker.cursor_sprint_id(),
        cursor_before
    );
    assert_eq!(
        app.dialog_input.assign_sprint_picker.selected_sprint_id(),
        selected_before
    );
}

#[test]
fn test_handle_carry_over_sprint_popup_with_a_not_loaded_sprint_tier_declines() {
    let mut app = App::test_default();
    seed_model_with_board(&mut app, LoadState::NotLoaded, LoadState::NotLoaded);
    let source_id = Uuid::new_v4();
    app.dialog_input.carry_over_source_sprint_id = Some(source_id);
    let selection_before = app.dialog_input.carry_over_sprint_selection.get();

    app.handle_carry_over_sprint_popup(KeyCode::Down);

    assert_error_banner_mentions(&app, "not loaded");
    assert_eq!(
        app.dialog_input.carry_over_sprint_selection.get(),
        selection_before
    );
}

#[test]
fn test_handle_carry_over_sprint_popup_distinguishes_missing_from_not_loaded() {
    let mut app = App::test_default();
    seed_model_with_board(&mut app, LoadState::NotLoaded, LoadState::Loaded(vec![]));
    let source_id = Uuid::new_v4();
    app.dialog_input.carry_over_source_sprint_id = Some(source_id);
    app.handle_carry_over_sprint_popup(KeyCode::Down);
    assert_no_banner(&app);

    let mut app = App::test_default();
    seed_model_with_board(&mut app, LoadState::NotLoaded, LoadState::NotLoaded);
    let source_id = Uuid::new_v4();
    app.dialog_input.carry_over_source_sprint_id = Some(source_id);
    app.handle_carry_over_sprint_popup(KeyCode::Down);
    assert_error_banner_mentions(&app, "not loaded");
}
