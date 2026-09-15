mod helpers;

use crossterm::event::KeyCode;
use helpers::{CountingBackend, ReadOp, ReadOpLog};
use kanban_core::Editable;
use kanban_domain::{
    commands::{CardCommand, Command, DeleteCard},
    BoardSettingsDto, CardMetadataDto, CreateCardOptions, EntityIds, KanbanOperations,
};
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::AppMode;
use kanban_tui::app::BoardFocus;
use kanban_tui::App;
use uuid::Uuid;

struct Seed {
    board: Uuid,
    c1: Uuid,
    c2: Uuid,
    k1: Uuid,
    k2: Uuid,
}

fn seed_two_columns_two_cards(app: &mut App) -> Seed {
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let c1 = app
        .ctx
        .create_column(board.id, "C1".to_string(), Some(0))
        .unwrap();
    let c2 = app
        .ctx
        .create_column(board.id, "C2".to_string(), Some(1))
        .unwrap();
    let k1 = app
        .ctx
        .create_card(
            board.id,
            c1.id,
            "K1".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let k2 = app
        .ctx
        .create_card(
            board.id,
            c2.id,
            "K2".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    Seed {
        board: board.id,
        c1: c1.id,
        c2: c2.id,
        k1: k1.id,
        k2: k2.id,
    }
}

/// Wraps the backend in a `CountingBackend`, runs `load_initial_state`
/// (the scope-priming idiom every test in this module must use, never
/// `reload_model`), and clears the op log so only the handler-under-test's
/// reads are recorded.
async fn prime(app: &mut App) -> ReadOpLog {
    let (backend, _reads, ops) = CountingBackend::wrap(app.ctx.backend());
    app.ctx.replace_backend(backend);
    app.load_initial_state().await;
    ops.lock().unwrap().clear();
    ops
}

/// Narrows the raw op log to the refetch-shaped reads: `snapshot`,
/// `get_graph`, and every `list_*` except `list_prefixes` (issued by the
/// create_card builder as part of the mutation itself, not the refetch).
fn refetch_ops(log: &ReadOpLog) -> Vec<ReadOp> {
    log.lock()
        .unwrap()
        .iter()
        .filter(|op| {
            (op.method == "snapshot" || op.method == "get_graph" || op.method.starts_with("list_"))
                && op.method != "list_prefixes"
        })
        .cloned()
        .collect()
}

fn has_op(ops: &[ReadOp], method: &str) -> bool {
    ops.iter().any(|op| op.method == method)
}

fn has_op_with_id(ops: &[ReadOp], method: &str, id: Uuid) -> bool {
    ops.iter()
        .any(|op| op.method == method && op.ids.contains(&id))
}

fn select_column(app: &mut App, board_id: Uuid, column_id: Uuid) {
    app.focus.board_focus = BoardFocus::Columns;
    let columns = kanban_domain::card_lifecycle::sorted_board_columns(
        board_id,
        app.model
            .board_columns_state(board_id)
            .loaded()
            .copied()
            .unwrap_or(&[]),
    );
    let idx = columns
        .iter()
        .position(|c| c.id == column_id)
        .expect("column visible");
    app.dialog_input
        .column_list
        .update_item_count(columns.len());
    app.dialog_input.column_list.set_selected_index(Some(idx));
}

#[tokio::test]
async fn test_rename_column_refetches_the_column_tiers_without_a_snapshot() {
    let mut app = App::test_default();
    let seed = seed_two_columns_two_cards(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    select_column(&mut app, seed.board, seed.c1);
    app.input.set("Renamed".to_string());
    app.rename_column();

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(
        has_op_with_id(&refetch, "list_columns_by_board", seed.board),
        "got {refetch:?}"
    );
    assert!(
        has_op_with_id(&refetch, "list_cards_by_column", seed.c1),
        "got {refetch:?}"
    );

    assert!(!has_op(&refetch, "list_all_cards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "get_graph"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_archived_cards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_archived_boards"), "got {refetch:?}");
    assert!(
        !has_op_with_id(&refetch, "list_cards_by_column", seed.c2),
        "got {refetch:?}"
    );

    assert!(app.model.column_cards_state(seed.c2).is_loaded());
    assert!(app.model.column_cards_state(seed.c1).is_loaded());
}

#[tokio::test]
async fn test_delete_column_refetches_the_card_tier_because_the_batch_moved_cards() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let c1 = app
        .ctx
        .create_column(board.id, "C1".to_string(), Some(0))
        .unwrap();
    let c2 = app
        .ctx
        .create_column(board.id, "C2".to_string(), Some(1))
        .unwrap();
    let c3 = app
        .ctx
        .create_column(board.id, "C3".to_string(), Some(2))
        .unwrap();
    for i in 0..3 {
        app.ctx
            .create_card(
                board.id,
                c2.id,
                format!("K{i}"),
                CreateCardOptions::default(),
            )
            .unwrap();
    }
    for i in 0..2 {
        app.ctx
            .create_card(
                board.id,
                c3.id,
                format!("M{i}"),
                CreateCardOptions::default(),
            )
            .unwrap();
    }

    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(board.id);
    select_column(&mut app, board.id, c2.id);
    app.delete_column();

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(
        has_op_with_id(&refetch, "list_columns_by_board", board.id),
        "got {refetch:?}"
    );
    assert!(has_op(&refetch, "list_cards_by_column"), "got {refetch:?}");

    let moved: Vec<_> = app
        .model
        .column_cards_state(c1.id)
        .loaded()
        .copied()
        .unwrap_or(&[])
        .iter()
        .filter(|card| card.column_id == c1.id)
        .collect();
    assert_eq!(
        moved.len(),
        3,
        "the three cards moved out of c2 must all report c1"
    );

    let untouched: Vec<_> = app
        .model
        .column_cards_state(c3.id)
        .loaded()
        .copied()
        .unwrap_or(&[])
        .iter()
        .filter(|card| card.column_id == c3.id)
        .collect();
    let mut untouched_titles: Vec<_> = untouched.iter().map(|card| card.title.clone()).collect();
    untouched_titles.sort();
    assert_eq!(
        untouched_titles,
        vec!["M0".to_string(), "M1".to_string()],
        "c3's own cards must be untouched by the c2 -> c1 move"
    );
}

#[tokio::test]
async fn test_move_column_up_leaves_an_untouched_columns_card_scope_loaded() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let c1 = app
        .ctx
        .create_column(board.id, "C1".to_string(), Some(0))
        .unwrap();
    let c2 = app
        .ctx
        .create_column(board.id, "C2".to_string(), Some(1))
        .unwrap();
    let c3 = app
        .ctx
        .create_column(board.id, "C3".to_string(), Some(2))
        .unwrap();
    for (col, name) in [(&c1, "k1"), (&c2, "k2"), (&c3, "k3")] {
        app.ctx
            .create_card(
                board.id,
                col.id,
                name.to_string(),
                CreateCardOptions::default(),
            )
            .unwrap();
    }

    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(board.id);
    app.mode = AppMode::BoardDetail;
    select_column(&mut app, board.id, c3.id);
    app.handle_move_column_up();

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(
        has_op_with_id(&refetch, "list_columns_by_board", board.id),
        "got {refetch:?}"
    );
    assert!(
        has_op_with_id(&refetch, "list_cards_by_column", c2.id),
        "got {refetch:?}"
    );
    assert!(
        has_op_with_id(&refetch, "list_cards_by_column", c3.id),
        "got {refetch:?}"
    );
    assert!(
        !has_op_with_id(&refetch, "list_cards_by_column", c1.id),
        "got {refetch:?}"
    );
    assert!(app.model.column_cards_state(c1.id).is_loaded());
}

#[tokio::test]
async fn test_move_column_down_leaves_an_untouched_columns_card_scope_loaded() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let c1 = app
        .ctx
        .create_column(board.id, "C1".to_string(), Some(0))
        .unwrap();
    let c2 = app
        .ctx
        .create_column(board.id, "C2".to_string(), Some(1))
        .unwrap();
    let c3 = app
        .ctx
        .create_column(board.id, "C3".to_string(), Some(2))
        .unwrap();
    for (col, name) in [(&c1, "k1"), (&c2, "k2"), (&c3, "k3")] {
        app.ctx
            .create_card(
                board.id,
                col.id,
                name.to_string(),
                CreateCardOptions::default(),
            )
            .unwrap();
    }

    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(board.id);
    app.mode = AppMode::BoardDetail;
    select_column(&mut app, board.id, c1.id);
    app.handle_move_column_down();

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(
        has_op_with_id(&refetch, "list_columns_by_board", board.id),
        "got {refetch:?}"
    );
    assert!(
        has_op_with_id(&refetch, "list_cards_by_column", c1.id),
        "got {refetch:?}"
    );
    assert!(
        has_op_with_id(&refetch, "list_cards_by_column", c2.id),
        "got {refetch:?}"
    );
    assert!(
        !has_op_with_id(&refetch, "list_cards_by_column", c3.id),
        "got {refetch:?}"
    );
    assert!(app.model.column_cards_state(c3.id).is_loaded());
}

#[tokio::test]
async fn test_create_column_refetches_the_column_tier_and_the_new_columns_cards() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let _c1 = app
        .ctx
        .create_column(board.id, "C1".to_string(), Some(0))
        .unwrap();

    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(board.id);
    app.focus.board_focus = BoardFocus::Columns;
    app.input.set("New Column".to_string());
    app.create_column();

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(
        has_op_with_id(&refetch, "list_columns_by_board", board.id),
        "got {refetch:?}"
    );
    assert!(has_op(&refetch, "list_cards_by_column"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_cards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "get_graph"), "got {refetch:?}");
}

#[tokio::test]
async fn test_rename_board_refetches_the_board_list_and_the_board_column_scope_only() {
    let mut app = App::test_default();
    let seed = seed_two_columns_two_cards(&mut app);
    let ops = prime(&mut app).await;

    app.board_list.select_board(seed.board);
    app.input.set("Renamed Board".to_string());
    app.rename_board();

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(
        has_op_with_id(&refetch, "list_columns_by_board", seed.board),
        "got {refetch:?}"
    );
    assert!(!has_op(&refetch, "list_all_columns"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_cards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");
    assert!(!has_op(&refetch, "get_graph"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_archived_cards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_archived_boards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_cards_by_column"), "got {refetch:?}");
}

#[tokio::test]
async fn test_toggle_sort_order_refetches_the_board_list_without_a_snapshot() {
    let mut app = App::test_default();
    let seed = seed_two_columns_two_cards(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.focus.active = Focus::Cards;
    app.filter.current_sort_order = Some(kanban_domain::SortOrder::Ascending);
    app.filter.current_sort_field = Some(kanban_domain::SortField::Position);
    app.handle_toggle_sort_order_key();

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(has_op(&refetch, "list_boards"), "got {refetch:?}");
}

#[tokio::test]
async fn test_select_task_list_view_refetches_the_board_list_without_a_snapshot() {
    let mut app = App::test_default();
    let seed = seed_two_columns_two_cards(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.dialog_input.task_list_view_selection.set(Some(0));
    app.handle_select_task_list_view_popup(KeyCode::Enter);

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(has_op(&refetch, "list_boards"), "got {refetch:?}");
}

#[tokio::test]
async fn test_create_board_refetches_the_board_and_column_tiers_without_a_snapshot() {
    let mut app = App::test_default();
    let _seed = seed_two_columns_two_cards(&mut app);
    let ops = prime(&mut app).await;

    app.focus.active = Focus::Boards;
    app.input.set("New Board".to_string());
    app.create_board();

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(has_op(&refetch, "list_columns_by_board"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_cards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "get_graph"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_archived_cards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_archived_boards"), "got {refetch:?}");
}

#[tokio::test]
async fn test_toggle_card_completion_refetches_the_card_tiers_and_repairs_the_visible_column_scopes(
) {
    let mut app = App::test_default();
    let seed = seed_two_columns_two_cards(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.focus.active = Focus::Cards;
    app.selection.active_card_id = Some(seed.k1);
    app.handle_toggle_card_completion();

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(
        has_op_with_id(&refetch, "list_cards_by_column", seed.c1),
        "got {refetch:?}"
    );
    assert!(
        has_op_with_id(&refetch, "list_cards_by_column", seed.c2),
        "got {refetch:?}"
    );

    assert!(!has_op(&refetch, "list_all_columns"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "get_graph"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_archived_cards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_archived_boards"), "got {refetch:?}");

    assert!(app.model.column_cards_state(seed.c1).is_loaded());
    assert!(app.model.column_cards_state(seed.c2).is_loaded());
}

#[tokio::test]
async fn test_a_card_mutation_repairs_the_column_scope_without_reading_the_whole_card_list() {
    let mut app = App::test_default();
    let seed = seed_two_columns_two_cards(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.focus.active = Focus::Cards;
    app.selection.active_card_id = Some(seed.k1);
    app.handle_toggle_card_completion();

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "list_all_cards"), "got {refetch:?}");
    assert!(
        has_op_with_id(&refetch, "list_cards_by_column", seed.c1),
        "got {refetch:?}"
    );
    assert!(app.model.column_cards_state(seed.c1).is_loaded());
}

#[tokio::test]
async fn test_move_card_refetches_the_card_tiers_and_leaves_the_column_tier_untouched() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let c1 = app
        .ctx
        .create_column(board.id, "C1".to_string(), Some(0))
        .unwrap();
    let c2 = app
        .ctx
        .create_column(board.id, "C2".to_string(), Some(1))
        .unwrap();
    let mut last = None;
    for i in 0..3 {
        last = Some(
            app.ctx
                .create_card(
                    board.id,
                    c1.id,
                    format!("K{i}"),
                    CreateCardOptions::default(),
                )
                .unwrap(),
        );
    }
    let card = last.unwrap();

    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Cards;
    app.selection.active_card_id = Some(card.id);
    app.handle_move_card_right();

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(
        has_op_with_id(&refetch, "list_cards_by_column", c1.id),
        "got {refetch:?}"
    );
    assert!(
        has_op_with_id(&refetch, "list_cards_by_column", c2.id),
        "got {refetch:?}"
    );
    assert!(!has_op(&refetch, "list_all_columns"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");
    assert!(!has_op(&refetch, "get_graph"), "got {refetch:?}");
}

#[tokio::test]
async fn test_toggle_selected_cards_completion_refetches_once_for_the_whole_batch() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let c1 = app
        .ctx
        .create_column(board.id, "C1".to_string(), Some(0))
        .unwrap();
    let mut ids = Vec::new();
    for i in 0..4 {
        let card = app
            .ctx
            .create_card(
                board.id,
                c1.id,
                format!("K{i}"),
                CreateCardOptions::default(),
            )
            .unwrap();
        ids.push(card.id);
    }

    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(board.id);
    app.focus.active = Focus::Cards;
    app.multi_select.selected_cards.insert(ids[0]);
    app.multi_select.selected_cards.insert(ids[1]);
    app.handle_toggle_card_completion();

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    let list_cards_by_column_count = refetch
        .iter()
        .filter(|op| op.method == "list_cards_by_column")
        .count();
    assert_eq!(list_cards_by_column_count, 1, "got {refetch:?}");
    assert!(
        has_op_with_id(&refetch, "list_cards_by_column", c1.id),
        "got {refetch:?}"
    );
}

#[tokio::test]
async fn test_toggle_selected_cards_completion_with_no_resolvable_cards_issues_no_refetch_read() {
    let mut app = App::test_default();
    let seed = seed_two_columns_two_cards(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.focus.active = Focus::Cards;
    app.multi_select.selected_cards.insert(Uuid::new_v4());
    app.handle_toggle_card_completion();

    let refetch = refetch_ops(&ops);
    assert!(refetch.is_empty(), "got {refetch:?}");
}

#[tokio::test]
async fn test_move_selected_cards_with_no_resolvable_cards_issues_no_refetch_read() {
    let mut app = App::test_default();
    let seed = seed_two_columns_two_cards(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.focus.active = Focus::Cards;
    app.multi_select.selected_cards.insert(Uuid::new_v4());
    app.handle_move_card_right();

    let refetch = refetch_ops(&ops);
    assert!(refetch.is_empty(), "got {refetch:?}");
}

#[tokio::test]
async fn test_toggle_completion_for_card_ids_refetches_the_card_tiers_without_a_snapshot() {
    let mut app = App::test_default();
    let seed = seed_two_columns_two_cards(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.toggle_completion_for_card_ids(vec![seed.k1, seed.k2]);

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(
        has_op_with_id(&refetch, "list_cards_by_column", seed.c1),
        "got {refetch:?}"
    );
    assert!(
        has_op_with_id(&refetch, "list_cards_by_column", seed.c2),
        "got {refetch:?}"
    );
}

#[tokio::test]
async fn test_toggle_completion_for_card_ids_with_no_resolvable_cards_issues_no_refetch_read() {
    let mut app = App::test_default();
    let seed = seed_two_columns_two_cards(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.toggle_completion_for_card_ids(vec![Uuid::new_v4()]);

    let refetch = refetch_ops(&ops);
    assert!(refetch.is_empty(), "got {refetch:?}");
}

#[tokio::test]
async fn test_create_card_refetches_the_card_tiers_named_by_its_inverse() {
    let card_id = Uuid::new_v4();
    let delete = Command::Card(CardCommand::Delete(DeleteCard { card_id }));
    assert_eq!(
        delete.touched_entities(),
        Some(EntityIds::cards([card_id]).with_graph())
    );

    let mut app = App::test_default();
    let seed = seed_two_columns_two_cards(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.focus.active = Focus::Cards;
    app.input.set("New Card".to_string());
    app.create_card();

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(has_op(&refetch, "list_cards_by_column"), "got {refetch:?}");
    assert!(
        !has_op_with_id(&refetch, "list_columns_by_board", seed.board),
        "got {refetch:?}"
    );
    assert!(
        !has_op_with_id(&refetch, "list_sprints_by_board", seed.board),
        "got {refetch:?}"
    );

    assert!(!has_op(&refetch, "get_graph"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_archived_cards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_archived_boards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_columns"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_cards"), "got {refetch:?}");
}

#[tokio::test]
async fn test_apply_board_settings_failure_issues_no_refetch_read() {
    let mut app = App::test_default();
    let _seed = seed_two_columns_two_cards(&mut app);
    let ops = prime(&mut app).await;

    let dto = BoardSettingsDto {
        sprint_prefix: None,
        card_prefix: None,
        sprint_duration_days: None,
        sprint_names: Vec::new(),
    };
    app.apply_board_settings(Uuid::new_v4(), dto);

    let refetch = refetch_ops(&ops);
    assert!(refetch.is_empty(), "got {refetch:?}");
    assert!(app.ui_state.banner.is_some());
}

#[tokio::test]
async fn test_apply_board_settings_success_refetches_the_board_list_only() {
    let mut app = App::test_default();
    let seed = seed_two_columns_two_cards(&mut app);
    let ops = prime(&mut app).await;

    let board = app
        .model
        .board_by_id_state(seed.board)
        .loaded()
        .copied()
        .unwrap()
        .clone();
    let dto = BoardSettingsDto::from_entity(&board);
    app.apply_board_settings(seed.board, dto);

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(
        has_op_with_id(&refetch, "list_columns_by_board", seed.board),
        "got {refetch:?}"
    );
    assert!(!has_op(&refetch, "list_all_cards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_columns"), "got {refetch:?}");
    assert!(!has_op(&refetch, "get_graph"), "got {refetch:?}");
}

#[tokio::test]
async fn test_apply_card_metadata_failure_issues_no_refetch_read() {
    let mut app = App::test_default();
    let _seed = seed_two_columns_two_cards(&mut app);
    let ops = prime(&mut app).await;

    let dto = CardMetadataDto {
        priority: "Medium".to_string(),
        status: "Todo".to_string(),
        points: None,
        due_date: None,
    };
    app.apply_card_metadata(Uuid::new_v4(), dto);

    let refetch = refetch_ops(&ops);
    assert!(refetch.is_empty(), "got {refetch:?}");
    assert!(app.ui_state.banner.is_some());
}

#[tokio::test]
async fn test_apply_card_metadata_success_refetches_the_card_tiers() {
    let mut app = App::test_default();
    let seed = seed_two_columns_two_cards(&mut app);
    let ops = prime(&mut app).await;

    let card = app
        .model
        .board_cards_state(seed.board)
        .loaded()
        .and_then(|cards| cards.iter().find(|c| c.id == seed.k1))
        .cloned()
        .unwrap();
    let dto = CardMetadataDto::from_entity(card);
    app.apply_card_metadata(seed.k1, dto);

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(
        has_op_with_id(&refetch, "list_cards_by_column", seed.c1),
        "got {refetch:?}"
    );
    assert!(!has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_columns"), "got {refetch:?}");
    assert!(!has_op(&refetch, "get_graph"), "got {refetch:?}");
}

#[test]
fn test_no_converted_card_board_or_column_handler_still_reloads_wholesale() {
    let card_handlers = include_str!("../src/handlers/card_handlers.rs");
    let detail_view_handlers = include_str!("../src/handlers/detail_view_handlers.rs");
    let board_handlers = include_str!("../src/handlers/board_handlers.rs");
    let column_handlers = include_str!("../src/handlers/column_handlers.rs");

    fn production_reload_model_count(src: &str) -> usize {
        let boundary = src.find("#[cfg(test)]").unwrap_or(src.len());
        src[..boundary].matches("reload_model()").count()
    }

    assert_eq!(production_reload_model_count(card_handlers), 1);
    assert_eq!(production_reload_model_count(detail_view_handlers), 0);
    assert_eq!(production_reload_model_count(board_handlers), 3);
    assert_eq!(production_reload_model_count(column_handlers), 0);

    let card_handlers_boundary = card_handlers
        .find("#[cfg(test)]")
        .unwrap_or(card_handlers.len());
    let entity_ids_count = card_handlers[..card_handlers_boundary]
        .matches("EntityIds")
        .count();
    assert_eq!(
        entity_ids_count, 1,
        "the only production EntityIds construction left is create_card's with_prefixes() extra"
    );

    for src in [detail_view_handlers, board_handlers, column_handlers] {
        let boundary = src.find("#[cfg(test)]").unwrap_or(src.len());
        assert!(
            !src[..boundary].contains("EntityIds"),
            "handler built its own invalidation"
        );
    }
    for src in [
        card_handlers,
        detail_view_handlers,
        board_handlers,
        column_handlers,
    ] {
        let boundary = src.find("#[cfg(test)]").unwrap_or(src.len());
        assert!(
            !src[..boundary].contains("invalidation_from_inverse"),
            "handler built its own invalidation"
        );
    }
}
