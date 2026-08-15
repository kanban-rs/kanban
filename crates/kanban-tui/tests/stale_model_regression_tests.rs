use kanban_domain::KanbanOperations;
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::AppMode;
use kanban_tui::app::BoardFocus;
use kanban_tui::App;

fn setup_app_with_board() -> App {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let _col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    app.reload_model();
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

#[test]
fn test_create_board_assigns_correct_id_to_columns() {
    let mut app = App::test_default();
    app.focus.active = Focus::Boards;

    app.input.set("New Board".to_string());
    app.create_board();
    app.prepare_frame();

    let boards = app.model.boards();
    assert_eq!(boards.len(), 1, "should have exactly one board");
    let board_id = boards[0].id;

    let columns = app.model.columns();
    let board_columns: Vec<_> = columns.iter().filter(|c| c.board_id == board_id).collect();
    assert_eq!(
        board_columns.len(),
        3,
        "new board should have 3 default columns"
    );
}

#[test]
fn test_create_board_selects_new_board() {
    let mut app = App::test_default();

    // Create first board via ctx so it's a known baseline
    app.ctx.create_board("First".to_string(), None).unwrap();
    app.reload_model();
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.focus.active = Focus::Boards;

    // Create second board via handler
    app.input.set("Second".to_string());
    app.create_board();
    app.prepare_frame();

    let boards = app.model.boards();
    assert_eq!(boards.len(), 2);

    let selected = app.board_list.get_selected_index();
    assert_eq!(selected, Some(1), "selection should point to the new board");
    assert_eq!(boards[selected.unwrap()].name, "Second");
}

#[test]
fn test_create_card_selects_newly_created_card() {
    let mut app = setup_app_with_board();

    app.focus.active = Focus::Cards;
    app.input.set("My Card".to_string());
    app.create_card();
    app.prepare_frame();

    let selected_id = app.get_selected_card_id();
    assert!(
        selected_id.is_some(),
        "a card should be selected after creation"
    );

    let cards = app.model.all_cards();
    let created = cards.iter().find(|c| c.title == "My Card");
    assert!(created.is_some(), "card should exist in model");
    assert_eq!(selected_id.unwrap(), created.unwrap().id);
}

#[test]
fn test_create_card_selects_newly_created_card_when_prior_selection_exists() {
    // KAN-403 regression: when a card was already selected, creating a new
    // card must move the selector to the new card, not stay on the prior one.
    let mut app = setup_app_with_board();

    app.focus.active = Focus::Cards;
    app.input.set("First".to_string());
    app.create_card();
    app.prepare_frame();
    let first_id = app
        .get_selected_card_id()
        .expect("first card should be selected after creation");

    app.input.set("Second".to_string());
    app.create_card();
    app.prepare_frame();

    let cards = app.model.all_cards();
    let second = cards
        .iter()
        .find(|c| c.title == "Second")
        .expect("second card exists in model");
    assert_ne!(second.id, first_id);

    let selected = app
        .get_selected_card_id()
        .expect("a card is selected after the second creation");
    assert_eq!(
        selected, second.id,
        "selection must jump to the newly created card, not stay on the prior one"
    );
}

#[test]
fn test_create_card_auto_completes_in_done_column() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let _col1 = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    let _col2 = app
        .ctx
        .create_column(board.id, "Doing".to_string(), Some(1))
        .unwrap();
    let done_col = app
        .ctx
        .create_column(board.id, "Done".to_string(), Some(2))
        .unwrap();
    app.ctx
        .update_column(
            done_col.id,
            kanban_domain::ColumnUpdate {
                default_status: Some(Some(kanban_domain::CardStatus::Done)),
                ..Default::default()
            },
        )
        .unwrap();
    app.reload_model();
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);

    // Use ColumnView and navigate to the done column (index 2)
    app.focus.active = Focus::Cards;
    app.switch_view_strategy(kanban_domain::TaskListView::ColumnView);
    app.reload_model();
    app.prepare_frame();
    // Navigate right twice to reach the 3rd column (Done)
    app.view.strategy.navigate_right(false);
    app.view.strategy.navigate_right(false);

    app.input.set("Done Card".to_string());
    app.create_card();
    app.prepare_frame();

    let cards = app.model.all_cards();
    let done_card = cards
        .iter()
        .find(|c| c.title == "Done Card" && c.column_id == done_col.id);
    assert!(done_card.is_some(), "card should be in done column");
    assert_eq!(
        done_card.unwrap().status,
        kanban_domain::CardStatus::Done,
        "card in done column should be auto-completed"
    );
}

#[test]
fn test_create_sprint_selects_new_sprint() {
    let mut app = setup_app_with_board();
    app.push_mode(AppMode::BoardDetail);
    app.focus.board_focus = BoardFocus::Sprints;

    app.input.set("".to_string());
    app.create_sprint();
    app.prepare_frame();

    let sprints = app.model.sprints();
    assert_eq!(sprints.len(), 1, "should have one sprint");

    let selected = app.selection.sprint.get();
    assert_eq!(
        selected,
        Some(0),
        "selection should point to the new sprint"
    );
}

#[test]
fn test_create_column_selects_new_column() {
    let mut app = setup_app_with_board();
    app.push_mode(AppMode::BoardDetail);
    app.focus.board_focus = BoardFocus::Columns;

    let columns_before = app
        .model
        .columns()
        .iter()
        .filter(|c| c.board_id == app.model.boards()[0].id)
        .count();

    app.input.set("New Column".to_string());
    app.create_column();
    app.prepare_frame();

    let selected = app.dialog_input.column_list.get_selected_index();
    assert_eq!(
        selected,
        Some(columns_before),
        "selection should point to the newly created column"
    );
}

#[test]
fn test_complete_sole_planning_sprint_does_not_show_carry_over() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let _col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    app.reload_model();
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);

    // Create a single sprint (Planning status by default)
    app.push_mode(AppMode::BoardDetail);
    app.focus.board_focus = BoardFocus::Sprints;
    app.input.set("".to_string());
    app.create_sprint();
    app.prepare_frame();

    let sprint_id = app.model.sprints()[0].id;

    // Create a card and assign it to the sprint
    app.focus.active = Focus::Cards;
    app.input.set("Task".to_string());
    app.create_card();
    app.prepare_frame();

    let card_id = app
        .model
        .all_cards()
        .iter()
        .find(|c| c.title == "Task")
        .unwrap()
        .id;
    app.ctx.assign_card_to_sprint(card_id, sprint_id).unwrap();
    app.reload_model();
    app.prepare_frame();

    // Navigate to sprint detail and complete it
    app.selection.active_sprint_index = Some(0);
    app.handle_complete_sprint_key();
    app.prepare_frame();

    // The sole planning sprint was just completed — no other planning sprint exists,
    // so carry-over dialog must NOT open. Before the s.id != sprint_id fix, the stale
    // model still showed the completed sprint as Planning, falsely triggering carry-over.
    assert_eq!(
        app.dialog_input.carry_over_source_sprint_id, None,
        "carry-over dialog should not open when completing the only planning sprint"
    );
}

#[test]
fn test_complete_sprint_with_other_planning_sprint_shows_carry_over() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let _col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    app.reload_model();
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);

    // Create two sprints (both start as Planning)
    app.push_mode(AppMode::BoardDetail);
    app.focus.board_focus = BoardFocus::Sprints;
    app.input.set("".to_string());
    app.create_sprint();
    app.prepare_frame();
    app.input.set("".to_string());
    app.create_sprint();
    app.prepare_frame();

    assert_eq!(app.model.sprints().len(), 2, "should have two sprints");
    let sprint1_id = app.model.sprints()[0].id;

    // Activate sprint 1 so it can be completed
    app.selection.active_sprint_index = Some(0);
    app.handle_activate_sprint_key();
    app.prepare_frame();

    // Create a card and assign it to sprint 1
    app.focus.active = Focus::Cards;
    app.input.set("Task".to_string());
    app.create_card();
    app.prepare_frame();

    let card_id = app
        .model
        .all_cards()
        .iter()
        .find(|c| c.title == "Task")
        .unwrap()
        .id;
    app.ctx.assign_card_to_sprint(card_id, sprint1_id).unwrap();
    app.reload_model();
    app.prepare_frame();

    // Complete sprint 1 — sprint 2 is still Planning
    app.selection.active_sprint_index = Some(0);
    app.handle_complete_sprint_key();
    app.prepare_frame();

    // Carry-over dialog should open because sprint 2 is Planning and sprint 1 has uncompleted cards
    assert_eq!(
        app.dialog_input.carry_over_source_sprint_id,
        Some(sprint1_id),
        "carry-over dialog should open with sprint 1 as source"
    );
}

#[test]
fn test_move_card_right_selects_moved_card_in_kanban_view() {
    // KAN-437 regression: after h/l move, selector must follow the moved card
    // into the target column, not stay on a prior card there.
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let _todo = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    let _doing = app
        .ctx
        .create_column(board.id, "Doing".to_string(), Some(1))
        .unwrap();
    let _done = app
        .ctx
        .create_column(board.id, "Done".to_string(), Some(2))
        .unwrap();
    app.reload_model();
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.switch_view_strategy(kanban_domain::TaskListView::ColumnView);
    app.reload_model();
    app.prepare_frame();
    app.focus.active = Focus::Cards;

    // Create two cards in Todo so there is a "prior selection" in Doing's
    // task list that could clobber the moved card's selection.
    app.input.set("Anchor".to_string());
    app.create_card();
    app.prepare_frame();
    app.input.set("Mover".to_string());
    app.create_card();
    app.prepare_frame();
    let mover_id = app.get_selected_card_id().expect("Mover is selected");

    // Move "Mover" right: Todo -> Doing.
    app.handle_move_card_right();
    app.prepare_frame();

    let selected = app
        .get_selected_card_id()
        .expect("a card is selected after move");
    assert_eq!(
        selected, mover_id,
        "selector must follow the moved card into the target column"
    );
}

#[test]
fn test_move_selected_cards_right_selects_first_moved_card() {
    // KAN-437 regression: after multi-select h/l move, selector must follow
    // the first moved card into the target column.
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let _todo = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    let _doing = app
        .ctx
        .create_column(board.id, "Doing".to_string(), Some(1))
        .unwrap();
    let _done = app
        .ctx
        .create_column(board.id, "Done".to_string(), Some(2))
        .unwrap();
    app.reload_model();
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.switch_view_strategy(kanban_domain::TaskListView::ColumnView);
    app.reload_model();
    app.prepare_frame();
    app.focus.active = Focus::Cards;

    // Create three cards in Todo.
    app.input.set("Anchor".to_string());
    app.create_card();
    app.prepare_frame();
    app.input.set("First Mover".to_string());
    app.create_card();
    app.prepare_frame();
    let first_mover_id = app.get_selected_card_id().expect("First Mover is selected");
    app.input.set("Second Mover".to_string());
    app.create_card();
    app.prepare_frame();
    let second_mover_id = app
        .get_selected_card_id()
        .expect("Second Mover is selected");

    // Enter multi-select mode and select both movers.
    app.multi_select.selection_mode_active = true;
    app.multi_select.selected_cards.insert(first_mover_id);
    app.multi_select.selected_cards.insert(second_mover_id);

    // Move selected cards right: Todo -> Doing.
    app.handle_move_card_right();
    app.prepare_frame();

    let selected = app
        .get_selected_card_id()
        .expect("a card is selected after multi-move");
    assert!(
        selected == first_mover_id || selected == second_mover_id,
        "selector must follow one of the moved cards into the target column, got neither"
    );
}

#[test]
fn test_delete_column_adjusts_selection() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    app.ctx
        .create_column(board.id, "Col1".to_string(), Some(0))
        .unwrap();
    app.ctx
        .create_column(board.id, "Col2".to_string(), Some(1))
        .unwrap();
    app.ctx
        .create_column(board.id, "Col3".to_string(), Some(2))
        .unwrap();
    app.reload_model();
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.push_mode(AppMode::BoardDetail);
    app.focus.board_focus = BoardFocus::Columns;

    // Select the last column (index 2) and delete it
    app.dialog_input.column_list.update_item_count(3);
    app.dialog_input.column_list.set_selected_index(Some(2));
    app.delete_column();
    app.prepare_frame();

    let remaining = app
        .model
        .columns()
        .iter()
        .filter(|c| c.board_id == board.id)
        .count();
    assert_eq!(remaining, 2, "should have 2 columns remaining");

    let selected = app.dialog_input.column_list.get_selected_index();
    assert_eq!(
        selected,
        Some(1),
        "selection should adjust to last remaining column"
    );
}

#[test]
fn test_toggle_card_completion_retains_selection() {
    // Stale-model regression: `toggle_card_completion` must call `prepare_frame()`
    // before `select_card_by_id` so the view task list reflects the post-toggle
    // card layout.  In the kanban (column) view the card moves to a different
    // column list, so without the refresh the selection silently drops.
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let _todo = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    let _done = app
        .ctx
        .create_column(board.id, "Done".to_string(), Some(1))
        .unwrap();
    app.reload_model();
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);

    // Switch to the kanban (column) view so each column has its own CardList.
    // This is the view where stale-model causes select_card_by_id to silently fail.
    app.switch_view_strategy(kanban_domain::TaskListView::ColumnView);
    app.reload_model();
    app.prepare_frame();

    app.focus.active = Focus::Cards;
    app.input.set("Task".to_string());
    app.create_card();
    app.prepare_frame();

    let card_id = app
        .get_selected_card_id()
        .expect("card should be selected after creation");

    // Toggle completion -- service will auto-move the card to the Done column.
    // Without prepare_frame() inside toggle_card_completion (before select_card_by_id)
    // the next prepare_frame() call (simulating the next render frame) will move the
    // card to the Done column list while the selector still points to the Todo column,
    // silently dropping the selection.
    app.handle_toggle_card_completion();
    // Simulate the next render frame -- this is where the stale-model bug manifests.
    app.prepare_frame();

    let selected_after = app.get_selected_card_id();
    assert_eq!(
        selected_after,
        Some(card_id),
        "selection must remain on the toggled card even after it moves to the Done column"
    );
}

#[test]
fn test_toggle_multi_card_completion_retains_selection() {
    // Same stale-model regression for the multi-select toggle path in kanban view.
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let _todo = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    let _done = app
        .ctx
        .create_column(board.id, "Done".to_string(), Some(1))
        .unwrap();
    app.reload_model();
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);

    app.switch_view_strategy(kanban_domain::TaskListView::ColumnView);
    app.reload_model();
    app.prepare_frame();

    app.focus.active = Focus::Cards;
    app.input.set("Alpha".to_string());
    app.create_card();
    app.prepare_frame();
    let alpha_id = app
        .get_selected_card_id()
        .expect("alpha should be selected after creation");

    app.input.set("Beta".to_string());
    app.create_card();
    app.prepare_frame();
    let beta_id = app
        .get_selected_card_id()
        .expect("beta should be selected after creation");

    // Enter multi-select mode and select both cards.
    app.multi_select.selection_mode_active = true;
    app.multi_select.selected_cards.insert(alpha_id);
    app.multi_select.selected_cards.insert(beta_id);

    // Toggle completion for both -- both cards move to the Done column.
    app.handle_toggle_card_completion();
    // Simulate the next render frame -- this is where the stale-model bug manifests.
    app.prepare_frame();

    // After the toggle the first selected card (alpha or beta) must still be
    // reachable via get_selected_card_id; losing the selection is the bug.
    let selected_after = app.get_selected_card_id();
    assert!(
        selected_after.is_some(),
        "a card must remain selected after multi-select toggle completion"
    );
    assert!(
        selected_after == Some(alpha_id) || selected_after == Some(beta_id),
        "selection must be one of the toggled cards, got {:?}",
        selected_after
    );
}

#[test]
fn test_move_card_right_syncs_column_list_count_to_filtered_columns_not_raw_board_count() {
    // KAN-1093 follow-up: handle_move_card's cosmetic column_list bookkeeping
    // used to derive its item count from the raw, unfiltered board column
    // count, disagreeing with visible_board_columns (which every other
    // column call site resolves indices against once a column search is
    // active). Not reachable from Kanban view today (column search only
    // activates from the board-detail Columns panel), but a stale
    // column_search left active is still real synced state, so this pins the
    // count to agree with visible_board_columns regardless.
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let _todo = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    let _doing = app
        .ctx
        .create_column(board.id, "Doing".to_string(), Some(1))
        .unwrap();
    let _done = app
        .ctx
        .create_column(board.id, "Done".to_string(), Some(2))
        .unwrap();
    app.reload_model();
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    // `is_kanban_view()` reads the board's persisted `task_list_view`, not
    // the app's local view strategy — both must be set for
    // `handle_move_card`'s column_list tracking block to actually run.
    app.execute_command(kanban_domain::commands::Command::Board(
        kanban_domain::commands::BoardCommand::SetTaskListView(
            kanban_domain::commands::SetBoardTaskListView {
                board_id: board.id,
                view: kanban_domain::TaskListView::ColumnView,
            },
        ),
    ))
    .unwrap();
    app.switch_view_strategy(kanban_domain::TaskListView::ColumnView);
    app.reload_model();
    app.prepare_frame();
    app.focus.active = Focus::Cards;

    app.input.set("Mover".to_string());
    app.create_card();
    app.prepare_frame();

    // Narrow the column list to a single match — if the cosmetic tracking
    // used the raw 3-column count instead, this would silently disagree with
    // ListComponent's own bounds (which are set from the filtered count
    // everywhere else in this board's column handling).
    app.filter.column_search.activate();
    for c in "todo".chars() {
        app.filter.column_search.input.insert_char(c);
    }

    app.handle_move_card_right();
    app.prepare_frame();

    assert_eq!(
        app.dialog_input.column_list.len(),
        1,
        "column_list's item count after a card move must match visible_board_columns \
         (filtered to 1 by the active column search), not the raw 3-column board count"
    );
}
