use crossterm::event::KeyCode;
use kanban_domain::model::ModelLoadStates;
use kanban_domain::{Board, LoadState, Sprint};
use kanban_tui::app::dialog_input::CreateCardFocus;
use kanban_tui::components::banner::BannerVariant;
use kanban_tui::App;
use kanban_view::filters::{FilterDialogSection, FilterDialogState};
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
fn test_handle_create_card_dialog_with_a_not_loaded_sprint_tier_declines() {
    let mut app = App::test_default();
    let board_id = seed_model_with_board(&mut app, LoadState::Loaded(vec![]), LoadState::NotLoaded);
    app.selection.active_board_id = Some(board_id);
    app.dialog_input.create_card_focus = CreateCardFocus::Sprint;

    app.handle_create_card_dialog(KeyCode::Down);

    assert_error_banner_mentions(&app, "not loaded");
}

#[test]
fn test_handle_prefix_dialog_impl_with_a_not_loaded_sprint_tier_declines() {
    let mut app = App::test_default();
    seed_model_with_board(&mut app, LoadState::Loaded(vec![]), LoadState::NotLoaded);
    app.selection.active_sprint_id = Some(Uuid::new_v4());
    app.input.clear();

    app.handle_set_sprint_prefix_dialog(KeyCode::Enter);

    assert_error_banner_mentions(&app, "not loaded");
}

#[test]
fn test_handle_prefix_dialog_impl_distinguishes_missing_from_not_loaded() {
    let mut app = App::test_default();
    seed_model_with_board(
        &mut app,
        LoadState::Loaded(vec![]),
        LoadState::Loaded(vec![]),
    );
    app.selection.active_sprint_id = Some(Uuid::new_v4());
    app.input.clear();
    app.handle_set_sprint_prefix_dialog(KeyCode::Enter);
    assert_no_banner(&app);

    let mut app = App::test_default();
    seed_model_with_board(&mut app, LoadState::Loaded(vec![]), LoadState::NotLoaded);
    app.selection.active_sprint_id = Some(Uuid::new_v4());
    app.input.clear();
    app.handle_set_sprint_prefix_dialog(KeyCode::Enter);
    assert_error_banner_mentions(&app, "not loaded");
}

#[test]
fn test_handle_filter_options_popup_with_a_not_loaded_sprint_tier_declines() {
    let mut app = App::test_default();
    let board_id = seed_model_with_board(&mut app, LoadState::Loaded(vec![]), LoadState::NotLoaded);
    app.selection.active_board_id = Some(board_id);
    let filters = kanban_domain::CardFilters {
        show_unassigned_sprints: false,
        selected_sprint_ids: Default::default(),
        date_from: None,
        date_to: None,
        selected_tags: Default::default(),
    };
    let mut dialog_state = FilterDialogState::new(filters);
    dialog_state.current_section = FilterDialogSection::Sprints;
    app.filter.dialog_state = Some(dialog_state);

    app.handle_filter_options_popup(KeyCode::Down);

    assert_error_banner_mentions(&app, "not loaded");
    assert_eq!(
        app.filter
            .dialog_state
            .as_ref()
            .expect("dialog state should still be open")
            .item_selection,
        0
    );
}
