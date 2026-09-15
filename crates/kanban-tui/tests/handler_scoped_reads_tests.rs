//! Pins the TUI handlers to the board-scoped tiers
//! (`App::board_columns_view`/`board_sprints_view`, `Controller::live_cards`/
//! `archived_cards`, `Model::card_by_id_state`/`column_cards_state`).
//! Each test seeds a healthy scoped state via `reload_model` +
//! `prepare_frame` and proves the handler acts on it.

use kanban_domain::{CreateCardOptions, KanbanOperations};
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::App;
use uuid::Uuid;

fn select_card_in_active_task_list(app: &mut App, card_id: Uuid) {
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
fn test_handler_acts_on_scoped_tiers_while_flat_tiers_failed() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".into(), None)
        .unwrap();
    app.ctx.create_sprint(board.id, None, None).unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    app.selection.active_board_id = Some(board.id);
    app.push_mode(AppMode::BoardDetail);
    app.reload_model();
    app.pop_mode();
    app.prepare_frame();
    select_card_in_active_task_list(&mut app, card.id);
    app.focus.active = kanban_tui::app::Focus::Cards;

    app.handle_assign_to_sprint_key();

    assert_eq!(app.mode, AppMode::Dialog(DialogMode::AssignCardToSprint));
    assert!(
        app.ui_state.banner.is_none(),
        "banner: {:?}",
        app.ui_state.banner
    );
}

#[test]
fn test_move_card_uses_the_scoped_column_and_partition_reads() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let todo = app
        .ctx
        .create_column(board.id, "Todo".into(), Some(0))
        .unwrap();
    let doing = app
        .ctx
        .create_column(board.id, "Doing".into(), Some(1))
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            todo.id,
            "Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    app.selection.active_board_id = Some(board.id);
    app.reload_model();
    app.prepare_frame();
    select_card_in_active_task_list(&mut app, card.id);
    app.focus.active = kanban_tui::app::Focus::Cards;

    app.handle_move_card_right();

    assert!(
        app.ui_state.banner.is_none(),
        "banner: {:?}",
        app.ui_state.banner
    );
    let stored = app.ctx.data_store().get_card(card.id).unwrap().unwrap();
    assert_eq!(stored.column_id, doing.id);
}

#[test]
fn test_create_card_position_derives_from_the_column_tier() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".into(), Some(0))
        .unwrap();
    app.ctx
        .create_card(
            board.id,
            column.id,
            "First".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx
        .create_card(
            board.id,
            column.id,
            "Second".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    app.selection.active_board_id = Some(board.id);
    app.reload_model();
    app.prepare_frame();
    app.focus.active = kanban_tui::app::Focus::Cards;

    app.input.set("Third".into());
    app.create_card();

    assert!(
        app.ui_state.banner.is_none(),
        "banner: {:?}",
        app.ui_state.banner
    );
    let all = app.ctx.data_store().list_all_cards().unwrap();
    let created = all
        .iter()
        .find(|c| c.title == "Third")
        .expect("card was created");
    assert_eq!(created.position, 2);
}

#[test]
fn test_toggle_completion_resolves_cards_by_id() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".into(), None)
        .unwrap();
    let a = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "A".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let b = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "B".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    app.selection.active_board_id = Some(board.id);
    app.reload_model();
    app.prepare_frame();

    assert!(app.model.card_by_id_state(a.id).loaded().is_some());
    assert!(app.model.card_by_id_state(b.id).loaded().is_some());

    app.toggle_completion_for_card_ids(vec![a.id, b.id]);

    assert!(
        app.ui_state.banner.is_none(),
        "banner: {:?}",
        app.ui_state.banner
    );
    let stored_a = app.ctx.data_store().get_card(a.id).unwrap().unwrap();
    let stored_b = app.ctx.data_store().get_card(b.id).unwrap().unwrap();
    assert_eq!(stored_a.status, kanban_domain::CardStatus::Done);
    assert_eq!(stored_b.status, kanban_domain::CardStatus::Done);
}

fn seed_manage_children_fixture(app: &mut App) -> (Uuid, Uuid, Uuid, Uuid) {
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".into(), None)
        .unwrap();
    let target = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Target".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let live_candidate = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Live".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let archived_candidate = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Archived".into(),
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
fn test_manage_children_eligible_set_spans_archived_when_target_archived() {
    let mut app = App::test_default();
    let (board_id, target_id, live_id, archived_id) = seed_manage_children_fixture(&mut app);
    app.ctx.archive_card(target_id).unwrap();

    app.selection.active_board_id = Some(board_id);
    app.mode = AppMode::ArchivedCardsView;
    app.reload_model();
    app.prepare_frame();
    select_card_in_active_task_list(&mut app, target_id);

    app.handle_manage_children_from_list();

    assert!(
        app.relationship.card_ids.contains(&live_id),
        "live candidate must be eligible for an archived target"
    );
    assert!(
        app.relationship.card_ids.contains(&archived_id),
        "archived candidate must be eligible for an archived target"
    );
}

#[test]
fn test_manage_children_eligible_set_excludes_archived_when_target_live() {
    let mut app = App::test_default();
    let (board_id, target_id, live_id, archived_id) = seed_manage_children_fixture(&mut app);

    app.selection.active_board_id = Some(board_id);
    app.reload_model();
    app.prepare_frame();
    select_card_in_active_task_list(&mut app, target_id);

    app.handle_manage_children_from_list();

    assert!(
        app.relationship.card_ids.contains(&live_id),
        "live candidate must be eligible for a live target"
    );
    assert!(
        !app.relationship.card_ids.contains(&archived_id),
        "archived candidate must NOT be eligible for a live target"
    );
}

#[test]
fn test_sprint_dialogs_read_the_board_scoped_sprint_tier() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    app.ctx
        .create_column(board.id, "Todo".into(), None)
        .unwrap();
    app.ctx.create_sprint(board.id, None, None).unwrap();

    app.selection.active_board_id = Some(board.id);
    app.push_mode(AppMode::Dialog(DialogMode::CreateCard));
    app.reload_model();

    app.dialog_input.create_card_focus = kanban_tui::app::dialog_input::CreateCardFocus::Sprint;
    app.handle_create_card_dialog(crossterm::event::KeyCode::Down);

    assert!(
        app.ui_state.banner.is_none(),
        "banner: {:?}",
        app.ui_state.banner
    );
}

#[test]
fn test_the_tasks_panel_title_counts_from_the_scoped_tiers_while_the_flat_tiers_failed() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".into(), None)
        .unwrap();
    app.ctx.create_sprint(board.id, None, None).unwrap();
    app.ctx
        .create_card(
            board.id,
            column.id,
            "Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    app.selection.active_board_id = Some(board.id);
    app.focus.active = kanban_tui::app::Focus::Cards;
    app.reload_model();
    app.prepare_frame();

    let title = kanban_tui::ui::tasks_panel_title(&app, false);

    assert!(title.contains('1'), "title: {title}");
    assert!(!title.contains('\u{2026}'), "title: {title}");
}

#[test]
fn test_search_filters_the_scoped_card_tier_without_flat_tiers() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".into(), None)
        .unwrap();
    let alpha = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "alpha".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx
        .create_card(
            board.id,
            column.id,
            "beta".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    app.selection.active_board_id = Some(board.id);
    app.reload_model();
    app.prepare_frame();

    app.mode = AppMode::Search;
    app.filter.search.activate();
    for c in "alpha".chars() {
        app.filter.search.input.insert_char(c);
    }
    app.prepare_frame();

    let active_list = app
        .view
        .strategy
        .get_active_task_list()
        .expect("active task list");
    assert_eq!(active_list.cards, vec![alpha.id]);
}
