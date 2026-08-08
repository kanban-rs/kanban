//! KAN-435: sprint-detail card lists reuse the main-board `CardListComponent`.

use crossterm::event::KeyCode;
use kanban_domain::{
    BoardUpdate, CardPriority, CardStatus, CreateCardOptions, KanbanOperations, SortField,
    SortOrder,
};
use kanban_tui::app::sprint_view::SprintTaskPanel;
use kanban_tui::App;
use uuid::Uuid;

#[test]
fn test_sprint_detail_multi_select_works_on_completed_panel() {
    let mut app = App::test_default();
    let card_id = Uuid::new_v4();
    app.sprint_view
        .completed_component
        .toggle_multi_select(card_id);
    assert_eq!(
        app.sprint_view
            .completed_component
            .get_multi_selected()
            .len(),
        1,
        "multi-select must work on the Completed panel — Uncompleted/Completed parity"
    );
}

#[test]
fn test_sprint_detail_uncompleted_panel_supports_movement() {
    let app = App::test_default();
    assert!(
        app.sprint_view.uncompleted_component.config.allow_movement,
        "Uncompleted panel must allow movement actions for main-board parity"
    );
}

#[test]
fn test_sprint_detail_default_panel_is_uncompleted() {
    let app = App::test_default();
    assert_eq!(app.sprint_view.panel, SprintTaskPanel::Uncompleted);
}

#[test]
fn test_sprint_detail_uncompleted_panel_scrolls_when_selection_passes_viewport() {
    let mut app = App::test_default();
    let cards: Vec<Uuid> = (0..30).map(|_| Uuid::new_v4()).collect();
    app.sprint_view.uncompleted_cards.update_cards(cards);
    app.sprint_view
        .uncompleted_cards
        .set_selected_index(Some(0));

    for _ in 0..10 {
        app.sprint_view.uncompleted_cards.navigate_down();
    }
    assert_eq!(
        app.sprint_view.uncompleted_cards.get_scroll_offset(),
        0,
        "no scroll happens just from navigating — needs sync_scroll"
    );

    app.sprint_view.sync_scroll(5, 5);
    assert!(
        app.sprint_view.uncompleted_cards.get_scroll_offset() > 0,
        "after sync_scroll the offset must advance so the selection is visible"
    );
}

#[test]
fn test_sprint_detail_completed_panel_scrolls_independently_of_uncompleted() {
    let mut app = App::test_default();
    let unc: Vec<Uuid> = (0..30).map(|_| Uuid::new_v4()).collect();
    let comp: Vec<Uuid> = (0..30).map(|_| Uuid::new_v4()).collect();
    app.sprint_view.uncompleted_cards.update_cards(unc);
    app.sprint_view.completed_cards.update_cards(comp);

    app.sprint_view.completed_cards.set_selected_index(Some(20));
    app.sprint_view
        .uncompleted_cards
        .set_selected_index(Some(0));

    app.sprint_view.sync_scroll(5, 5);
    assert!(
        app.sprint_view.completed_cards.get_scroll_offset() > 0,
        "Completed panel must scroll based on its own selection"
    );
    assert_eq!(
        app.sprint_view.uncompleted_cards.get_scroll_offset(),
        0,
        "Uncompleted panel must not scroll if its selection is already visible"
    );
}

#[test]
fn test_sprint_detail_populate_applies_board_sort_order() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    app.ctx
        .update_board(
            board.id,
            BoardUpdate {
                task_sort_field: Some(SortField::Priority),
                task_sort_order: Some(SortOrder::Descending),
                ..Default::default()
            },
        )
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

    let sprint = app.ctx.create_sprint(board.id, None, None).unwrap();
    let low = app
        .ctx
        .create_card(
            board.id,
            col.id,
            "low priority".to_string(),
            CreateCardOptions {
                priority: Some(CardPriority::Low),
                ..Default::default()
            },
        )
        .unwrap();
    let critical = app
        .ctx
        .create_card(
            board.id,
            col.id,
            "critical priority".to_string(),
            CreateCardOptions {
                priority: Some(CardPriority::Critical),
                ..Default::default()
            },
        )
        .unwrap();
    let medium = app
        .ctx
        .create_card(
            board.id,
            col.id,
            "medium priority".to_string(),
            CreateCardOptions {
                priority: Some(CardPriority::Medium),
                ..Default::default()
            },
        )
        .unwrap();
    app.ctx.assign_card_to_sprint(low.id, sprint.id).unwrap();
    app.ctx
        .assign_card_to_sprint(critical.id, sprint.id)
        .unwrap();
    app.ctx.assign_card_to_sprint(medium.id, sprint.id).unwrap();
    app.prepare_frame();

    app.populate_sprint_task_lists(sprint.id);

    let ordered = &app.sprint_view.uncompleted_cards.cards;
    assert_eq!(
        ordered,
        &vec![critical.id, medium.id, low.id],
        "panel should be sorted by the board's active sort field (Priority Descending)"
    );
}

#[test]
fn test_sprint_detail_j_key_via_handler_advances_raw_card_list_selection() {
    let mut app = App::test_default();
    let cards: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();
    app.sprint_view
        .uncompleted_cards
        .update_cards(cards.clone());
    app.sprint_view
        .uncompleted_component
        .update_cards(cards.clone());
    app.sprint_view
        .uncompleted_cards
        .set_selected_index(Some(0));
    app.sprint_view
        .uncompleted_component
        .set_selected_index(Some(0));
    app.sprint_view.panel = SprintTaskPanel::Uncompleted;

    app.handle_sprint_detail_key(KeyCode::Char('j'));

    assert_eq!(
        app.sprint_view.uncompleted_cards.get_selected_index(),
        Some(1),
        "raw CardList selection must mirror component selection after key dispatch"
    );
}

#[test]
fn test_sprint_detail_status_done_card_is_not_in_uncompleted_panel_after_populate() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let col = app
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

    let sprint = app.ctx.create_sprint(board.id, None, None).unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            col.id,
            "done card".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.assign_card_to_sprint(card.id, sprint.id).unwrap();
    app.ctx
        .update_card(
            card.id,
            kanban_domain::CardUpdate {
                status: Some(CardStatus::Done),
                ..Default::default()
            },
        )
        .unwrap();
    app.prepare_frame();

    app.populate_sprint_task_lists(sprint.id);

    assert!(
        app.sprint_view.uncompleted_cards.cards.is_empty(),
        "a Done card must not appear in the Uncompleted panel"
    );
    assert_eq!(
        app.sprint_view.completed_cards.cards,
        vec![card.id],
        "a Done card must appear in the Completed panel"
    );
}
