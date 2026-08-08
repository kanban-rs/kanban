use crossterm::event::KeyCode;
use kanban_domain::KanbanOperations;
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::App;

fn setup_app_with_board() -> App {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let _col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app
}

fn board_id(app: &App) -> uuid::Uuid {
    app.model.boards()[0].id
}

fn confirm_create_card_dialog(app: &mut App, title: &str) {
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();
    assert!(matches!(app.mode, AppMode::Dialog(DialogMode::CreateCard)));
    for ch in title.chars() {
        app.handle_create_card_dialog(KeyCode::Char(ch));
    }
    app.handle_create_card_dialog(KeyCode::Enter);
    app.prepare_frame();
}

#[test]
fn test_create_card_dialog_auto_assigns_sole_active_sprint_on_open() {
    let mut app = setup_app_with_board();
    let bid = board_id(&app);
    let sprint = app.ctx.create_sprint(bid, None, None).unwrap();
    app.ctx.activate_sprint(sprint.id, Some(7)).unwrap();
    app.prepare_frame();

    confirm_create_card_dialog(&mut app, "Task");

    let cards = app.model.all_cards();
    let created = cards
        .iter()
        .find(|c| c.title == "Task")
        .expect("card created");
    assert_eq!(
        created.sprint_id,
        Some(sprint.id),
        "exactly one active sprint pre-checks it, so Enter without Space \
         confirms the assignment in one keystroke"
    );
}

#[test]
fn test_create_card_dialog_space_on_pre_checked_sprint_unchecks_it() {
    let mut app = setup_app_with_board();
    let bid = board_id(&app);
    let sprint = app.ctx.create_sprint(bid, None, None).unwrap();
    app.ctx.activate_sprint(sprint.id, Some(7)).unwrap();
    app.prepare_frame();

    app.focus.active = Focus::Cards;
    app.handle_create_card_key();
    for ch in "Task".chars() {
        app.handle_create_card_dialog(KeyCode::Char(ch));
    }
    // Sole active sprint was pre-checked on open. Tab into the picker
    // (cursor already on that sprint), Space toggles the check off.
    app.handle_create_card_dialog(KeyCode::Tab);
    app.handle_create_card_dialog(KeyCode::Char(' '));
    app.handle_create_card_dialog(KeyCode::Enter);
    app.prepare_frame();

    let cards = app.model.all_cards();
    let created = cards
        .iter()
        .find(|c| c.title == "Task")
        .expect("card created");
    assert_eq!(created.sprint_id, None);
}

#[test]
fn test_create_card_dialog_leaves_card_unassigned_when_no_active_sprint() {
    let mut app = setup_app_with_board();
    let bid = board_id(&app);
    // planning sprint exists but is not active
    let _planning = app.ctx.create_sprint(bid, None, None).unwrap();
    app.prepare_frame();

    confirm_create_card_dialog(&mut app, "Plain");

    let cards = app.model.all_cards();
    let created = cards
        .iter()
        .find(|c| c.title == "Plain")
        .expect("card created");
    assert_eq!(created.sprint_id, None);
}

#[test]
fn test_create_card_dialog_leaves_card_unassigned_when_multiple_active_sprints() {
    let mut app = setup_app_with_board();
    let bid = board_id(&app);
    let s1 = app.ctx.create_sprint(bid, None, None).unwrap();
    let s2 = app.ctx.create_sprint(bid, None, None).unwrap();
    app.ctx.activate_sprint(s1.id, Some(7)).unwrap();
    app.ctx.activate_sprint(s2.id, Some(7)).unwrap();
    app.prepare_frame();

    confirm_create_card_dialog(&mut app, "Ambig");

    let cards = app.model.all_cards();
    let created = cards
        .iter()
        .find(|c| c.title == "Ambig")
        .expect("card created");
    assert_eq!(
        created.sprint_id, None,
        "with multiple active sprints, no pre-selection so card stays unassigned"
    );
}

#[test]
fn test_tab_toggles_focus_between_title_and_sprint_picker() {
    let mut app = setup_app_with_board();
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();

    assert!(app.dialog_input.create_card_focus_is_title());
    app.handle_create_card_dialog(KeyCode::Tab);
    assert!(!app.dialog_input.create_card_focus_is_title());
    app.handle_create_card_dialog(KeyCode::Tab);
    assert!(app.dialog_input.create_card_focus_is_title());
}

#[test]
fn test_esc_on_title_focus_moves_focus_to_sprint_picker_without_closing() {
    let mut app = setup_app_with_board();
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();
    assert!(app.dialog_input.create_card_focus_is_title());

    app.handle_create_card_dialog(KeyCode::Esc);

    assert!(matches!(app.mode, AppMode::Dialog(DialogMode::CreateCard)));
    assert!(!app.dialog_input.create_card_focus_is_title());
}

#[test]
fn test_esc_on_sprint_focus_closes_the_dialog() {
    let mut app = setup_app_with_board();
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();
    app.handle_create_card_dialog(KeyCode::Tab);
    assert!(!app.dialog_input.create_card_focus_is_title());

    app.handle_create_card_dialog(KeyCode::Esc);

    assert!(
        !matches!(app.mode, AppMode::Dialog(DialogMode::CreateCard)),
        "Esc while sprint-focused should close the dialog"
    );
}

#[test]
fn test_down_on_title_focus_moves_focus_to_sprint_picker() {
    let mut app = setup_app_with_board();
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();
    assert!(app.dialog_input.create_card_focus_is_title());

    app.handle_create_card_dialog(KeyCode::Down);

    assert!(!app.dialog_input.create_card_focus_is_title());
}

#[test]
fn test_typing_on_sprint_focus_does_not_modify_title_input() {
    let mut app = setup_app_with_board();
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();
    app.handle_create_card_dialog(KeyCode::Tab);
    assert!(!app.dialog_input.create_card_focus_is_title());

    app.handle_create_card_dialog(KeyCode::Char('x'));
    assert_eq!(app.input.as_str(), "");
}

#[test]
fn test_j_on_sprint_focus_navigates_picker_like_down() {
    let mut app = setup_app_with_board();
    let bid = board_id(&app);
    let planning = app.ctx.create_sprint(bid, None, None).unwrap();
    app.prepare_frame();

    app.focus.active = Focus::Cards;
    app.handle_create_card_key();
    for ch in "Vim".chars() {
        app.handle_create_card_dialog(KeyCode::Char(ch));
    }
    app.handle_create_card_dialog(KeyCode::Tab);
    app.handle_create_card_dialog(KeyCode::Char('j'));
    app.handle_create_card_dialog(KeyCode::Char(' '));
    app.handle_create_card_dialog(KeyCode::Enter);
    app.prepare_frame();

    let cards = app.model.all_cards();
    let created = cards
        .iter()
        .find(|c| c.title == "Vim")
        .expect("card created");
    assert_eq!(created.sprint_id, Some(planning.id));
}

#[test]
fn test_j_on_title_focus_inserts_character_into_title() {
    let mut app = setup_app_with_board();
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();
    assert!(app.dialog_input.create_card_focus_is_title());

    app.handle_create_card_dialog(KeyCode::Char('j'));

    assert_eq!(app.input.as_str(), "j");
}

#[test]
fn test_arrow_to_none_row_then_space_explicitly_leaves_card_unassigned() {
    // From a sole-active board, walk the cursor up onto the (None) row
    // and press Space. The card lands unassigned even though there was
    // a candidate active sprint sitting under the initial cursor.
    let mut app = setup_app_with_board();
    let bid = board_id(&app);
    let sprint = app.ctx.create_sprint(bid, None, None).unwrap();
    app.ctx.activate_sprint(sprint.id, Some(7)).unwrap();
    app.prepare_frame();

    app.focus.active = Focus::Cards;
    app.handle_create_card_key();
    for ch in "NoSprint".chars() {
        app.handle_create_card_dialog(KeyCode::Char(ch));
    }
    app.handle_create_card_dialog(KeyCode::Tab);
    // Cursor starts on the active sprint; Up walks it to the (None) row.
    app.handle_create_card_dialog(KeyCode::Up);
    app.handle_create_card_dialog(KeyCode::Char(' '));
    app.handle_create_card_dialog(KeyCode::Enter);
    app.prepare_frame();

    let cards = app.model.all_cards();
    let created = cards
        .iter()
        .find(|c| c.title == "NoSprint")
        .expect("card created");
    assert_eq!(created.sprint_id, None);
}

#[test]
fn test_space_on_title_focus_inserts_space_into_title() {
    let mut app = setup_app_with_board();
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();
    assert!(app.dialog_input.create_card_focus_is_title());

    app.handle_create_card_dialog(KeyCode::Char('h'));
    app.handle_create_card_dialog(KeyCode::Char(' '));
    app.handle_create_card_dialog(KeyCode::Char('i'));

    assert_eq!(app.input.as_str(), "h i");
}

#[test]
fn test_arrow_down_then_space_assigns_navigated_sprint() {
    let mut app = setup_app_with_board();
    let bid = board_id(&app);
    let planning = app.ctx.create_sprint(bid, None, None).unwrap();
    app.prepare_frame();

    app.focus.active = Focus::Cards;
    app.handle_create_card_key();
    for ch in "Picked".chars() {
        app.handle_create_card_dialog(KeyCode::Char(ch));
    }
    app.handle_create_card_dialog(KeyCode::Tab);
    app.handle_create_card_dialog(KeyCode::Down);
    app.handle_create_card_dialog(KeyCode::Char(' '));
    app.handle_create_card_dialog(KeyCode::Enter);
    app.prepare_frame();

    let cards = app.model.all_cards();
    let created = cards
        .iter()
        .find(|c| c.title == "Picked")
        .expect("card created");
    assert_eq!(created.sprint_id, Some(planning.id));
}

#[test]
fn test_create_card_does_not_carry_sprint_id_from_a_different_board() {
    // Defense: the picker was reset for board A but by submit time the
    // active board is B. The selected sprint id belongs to A and would
    // otherwise leak into B's CreateCard command, surfacing only as a
    // cross-board validation error from the domain. The board-aware
    // accessor should drop the stale id before the command is built.
    let mut app = setup_app_with_board();
    let board_a = board_id(&app);
    let sprint_on_a = app.ctx.create_sprint(board_a, None, None).unwrap();
    app.ctx.activate_sprint(sprint_on_a.id, Some(7)).unwrap();

    let board_b = app.ctx.create_board("Board B".to_string(), None).unwrap();
    let col_b = app
        .ctx
        .create_column(board_b.id, "Todo".to_string(), Some(0))
        .unwrap();
    app.prepare_frame();

    // Reset picker for board A.
    let sprints = app.model.sprints().to_vec();
    let board_a_ref = app
        .model
        .boards()
        .iter()
        .find(|b| b.id == board_a)
        .cloned()
        .unwrap();
    app.dialog_input.create_card_sprint_picker.reset_for_board(
        &sprints,
        &board_a_ref,
        chrono::Utc::now(),
    );

    // Sanity: the picker holds board A's sprint id.
    assert_eq!(
        app.dialog_input
            .create_card_sprint_picker
            .selected_sprint_id(),
        Some(sprint_on_a.id)
    );
    // Board-aware accessor refuses to return it for board B.
    assert_eq!(
        app.dialog_input
            .create_card_sprint_picker
            .selected_sprint_id_for(board_b.id),
        None
    );
    // Ditto column on B exists for completeness of the scenario.
    let _ = col_b;
}
