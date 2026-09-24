//! Every test here drives its handler method directly, never through
//! `handle_key_event`, and never asserts a per-id `get_card`/`get_sprint`
//! op: `InvalidationPlan` only keeps a per-id tier that a prior per-id read
//! populated, and none of these code paths issue one.

mod helpers;

use crossterm::event::KeyCode;
use helpers::{CountingBackend, ReadOp, ReadOpLog};
use kanban_domain::commands::{Command, DeleteSprint, SprintCommand};
use kanban_domain::{CreateCardOptions, GraphOperations, KanbanOperations};
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::App;
use uuid::Uuid;

struct Seed {
    board: Uuid,
    c1: Uuid,
    c2: Uuid,
    k1: Uuid,
    k2: Uuid,
    sprint_a: Uuid,
}

fn seed_board_two_columns_two_cards_one_sprint(app: &mut App) -> Seed {
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
    let sprint_a = app.ctx.create_sprint(board.id, None, None).unwrap();
    Seed {
        board: board.id,
        c1: c1.id,
        c2: c2.id,
        k1: k1.id,
        k2: k2.id,
        sprint_a: sprint_a.id,
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
/// `get_graph`, and every `list_*` except `list_prefixes` (issued by
/// create-time builders as part of the mutation itself, not the refetch).
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
async fn test_activate_sprint_refetches_the_sprint_and_board_tiers_without_a_snapshot() {
    let mut app = App::test_default();
    let seed = seed_board_two_columns_two_cards_one_sprint(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.selection.active_sprint_id = Some(seed.sprint_a);
    app.mode = AppMode::BoardDetail;

    app.handle_activate_sprint_key();

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(
        has_op_with_id(&refetch, "list_columns_by_board", seed.board),
        "got {refetch:?}"
    );
    assert!(
        has_op_with_id(&refetch, "list_sprints_by_board", seed.board),
        "got {refetch:?}"
    );
    assert!(!has_op(&refetch, "list_all_cards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_columns"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_cards_by_column"), "got {refetch:?}");
    assert!(!has_op(&refetch, "get_graph"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_archived_cards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_archived_boards"), "got {refetch:?}");
}

#[tokio::test]
async fn test_complete_sprint_refetches_the_sprint_and_board_tiers_but_not_the_card_tier() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let c1 = app
        .ctx
        .create_column(board.id, "C1".to_string(), Some(0))
        .unwrap();
    let sprint = app.ctx.create_sprint(board.id, None, None).unwrap();
    app.ctx
        .activate_sprint(sprint.id, Some(14))
        .expect("activate");
    let k1 = app
        .ctx
        .create_card(
            board.id,
            c1.id,
            "K1".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.assign_card_to_sprint(k1.id, sprint.id).unwrap();
    let k2 = app
        .ctx
        .create_card(
            board.id,
            c1.id,
            "K2".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.assign_card_to_sprint(k2.id, sprint.id).unwrap();

    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(board.id);
    app.selection.active_sprint_id = Some(sprint.id);
    app.mode = AppMode::SprintDetail;

    app.handle_complete_sprint_key();

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_cards"), "got {refetch:?}");
}

#[tokio::test]
async fn test_create_sprint_falls_back_to_a_whole_model_reset_because_its_inverse_is_unenumerable()
{
    assert_eq!(
        Command::Sprint(SprintCommand::Delete(DeleteSprint {
            sprint_id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
        }))
        .touched_entities(),
        None
    );

    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(board.id);
    app.input.set("Alpha".to_string());
    app.create_sprint();

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(
        has_op_with_id(&refetch, "list_columns_by_board", board.id),
        "got {refetch:?}"
    );
    assert!(
        has_op_with_id(&refetch, "list_sprints_by_board", board.id),
        "got {refetch:?}"
    );
    assert!(!has_op(&refetch, "get_graph"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_archived_cards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_archived_boards"), "got {refetch:?}");
}

#[tokio::test]
async fn test_set_card_points_refetches_the_card_tiers_only() {
    let mut app = App::test_default();
    let seed = seed_board_two_columns_two_cards_one_sprint(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.selection.active_card_id = Some(seed.k1);
    app.input.set("3".to_string());
    app.handle_set_card_points_dialog(KeyCode::Enter);

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
    assert!(!has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_columns"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");
    assert!(!has_op(&refetch, "get_graph"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_archived_cards"), "got {refetch:?}");
}

#[tokio::test]
async fn test_set_card_points_failure_issues_no_refetch_read() {
    let mut app = App::test_default();
    let seed = seed_board_two_columns_two_cards_one_sprint(&mut app);

    let ops = prime(&mut app).await;
    app.ctx.data_store().delete_card(seed.k1).unwrap();

    app.selection.active_board_id = Some(seed.board);
    app.selection.active_card_id = Some(seed.k1);
    app.input.set("3".to_string());
    app.handle_set_card_points_dialog(KeyCode::Enter);

    let refetch = refetch_ops(&ops);
    assert!(refetch.is_empty(), "got {refetch:?}");
    assert!(app.ui_state.banner.is_some());
}

#[tokio::test]
async fn test_prefix_dialog_board_branches_refetch_the_board_list_and_never_the_sprint_list() {
    let mut app = App::test_default();
    let seed = seed_board_two_columns_two_cards_one_sprint(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.mode = AppMode::Dialog(DialogMode::SetBranchPrefix);

    app.input.set("ABC".to_string());
    app.handle_set_branch_prefix_dialog(KeyCode::Enter);

    {
        let refetch = refetch_ops(&ops);
        assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
        assert!(has_op(&refetch, "list_boards"), "got {refetch:?}");
        assert!(
            has_op_with_id(&refetch, "list_columns_by_board", seed.board),
            "got {refetch:?}"
        );
        assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");
        assert!(!has_op(&refetch, "list_all_cards"), "got {refetch:?}");
        assert!(!has_op(&refetch, "list_all_columns"), "got {refetch:?}");
        assert!(!has_op(&refetch, "list_cards_by_column"), "got {refetch:?}");
    }

    ops.lock().unwrap().clear();
    app.mode = AppMode::Dialog(DialogMode::SetBranchPrefix);
    app.input.set(String::new());
    app.handle_set_branch_prefix_dialog(KeyCode::Enter);

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(
        has_op_with_id(&refetch, "list_columns_by_board", seed.board),
        "got {refetch:?}"
    );
    assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_cards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_columns"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_cards_by_column"), "got {refetch:?}");
}

#[tokio::test]
async fn test_prefix_dialog_sprint_branches_refetch_the_sprint_list_and_never_the_board_list() {
    let mut app = App::test_default();
    let seed = seed_board_two_columns_two_cards_one_sprint(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.selection.active_sprint_id = Some(seed.sprint_a);
    app.mode = AppMode::Dialog(DialogMode::SetSprintPrefix);

    app.input.set("ABC".to_string());
    app.handle_set_sprint_prefix_dialog(KeyCode::Enter);

    {
        let refetch = refetch_ops(&ops);
        assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
        assert!(
            has_op_with_id(&refetch, "list_sprints_by_board", seed.board),
            "got {refetch:?}"
        );
        assert!(!has_op(&refetch, "list_boards"), "got {refetch:?}");
        assert!(!has_op(&refetch, "list_all_cards"), "got {refetch:?}");
        assert!(!has_op(&refetch, "list_all_columns"), "got {refetch:?}");
    }
    assert!(app.model.board_sprints_state(seed.board).is_loaded());

    ops.lock().unwrap().clear();
    app.mode = AppMode::Dialog(DialogMode::SetSprintPrefix);
    app.input.set(String::new());
    app.handle_set_sprint_prefix_dialog(KeyCode::Enter);

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(
        has_op_with_id(&refetch, "list_sprints_by_board", seed.board),
        "got {refetch:?}"
    );
    assert!(!has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_cards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_columns"), "got {refetch:?}");
}

#[tokio::test]
async fn test_prefix_dialog_sprint_card_branches_refetch_the_sprint_list_and_never_the_card_tier() {
    let mut app = App::test_default();
    let seed = seed_board_two_columns_two_cards_one_sprint(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.selection.active_sprint_id = Some(seed.sprint_a);
    app.mode = AppMode::Dialog(DialogMode::SetSprintCardPrefix);

    app.input.set("XYZ".to_string());
    app.handle_set_sprint_card_prefix_dialog(KeyCode::Enter);

    {
        let refetch = refetch_ops(&ops);
        assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
        assert!(!has_op(&refetch, "list_all_cards"), "got {refetch:?}");
        assert!(!has_op(&refetch, "list_cards_by_column"), "got {refetch:?}");
    }

    ops.lock().unwrap().clear();
    app.mode = AppMode::Dialog(DialogMode::SetSprintCardPrefix);
    app.input.set(String::new());
    app.handle_set_sprint_card_prefix_dialog(KeyCode::Enter);

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_cards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_cards_by_column"), "got {refetch:?}");
}

#[tokio::test]
async fn test_prefix_dialog_board_branch_failure_issues_no_refetch_read() {
    let mut app = App::test_default();
    let seed = seed_board_two_columns_two_cards_one_sprint(&mut app);

    let ops = prime(&mut app).await;
    app.ctx.data_store().delete_board(seed.board).unwrap();

    app.selection.active_board_id = Some(seed.board);
    app.mode = AppMode::Dialog(DialogMode::SetBranchPrefix);
    app.input.set("ABC".to_string());
    app.handle_set_branch_prefix_dialog(KeyCode::Enter);

    let refetch = refetch_ops(&ops);
    assert!(refetch.is_empty(), "got {refetch:?}");
    assert!(app.ui_state.banner.is_some());
}

#[tokio::test]
async fn test_prefix_dialog_sprint_branch_failure_issues_no_refetch_read() {
    let mut app = App::test_default();
    let seed = seed_board_two_columns_two_cards_one_sprint(&mut app);

    let ops = prime(&mut app).await;
    app.ctx.data_store().delete_sprint(seed.sprint_a).unwrap();

    app.selection.active_board_id = Some(seed.board);
    app.selection.active_sprint_id = Some(seed.sprint_a);
    app.mode = AppMode::Dialog(DialogMode::SetSprintPrefix);
    app.input.set("ABC".to_string());
    app.handle_set_sprint_prefix_dialog(KeyCode::Enter);

    let refetch = refetch_ops(&ops);
    assert!(refetch.is_empty(), "got {refetch:?}");
    assert!(app.ui_state.banner.is_some());
}

#[tokio::test]
async fn test_set_card_priority_refetches_the_card_tiers_only() {
    let mut app = App::test_default();
    let seed = seed_board_two_columns_two_cards_one_sprint(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.selection.active_card_id = Some(seed.k1);
    app.dialog_input.priority_selection.set(Some(3));
    app.handle_set_card_priority_popup(KeyCode::Enter);

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
    assert!(!has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_columns"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");
    assert!(!has_op(&refetch, "get_graph"), "got {refetch:?}");
}

#[tokio::test]
async fn test_set_card_priority_failure_issues_no_refetch_read() {
    let mut app = App::test_default();
    let seed = seed_board_two_columns_two_cards_one_sprint(&mut app);

    let ops = prime(&mut app).await;
    app.ctx.data_store().delete_card(seed.k1).unwrap();

    app.selection.active_board_id = Some(seed.board);
    app.selection.active_card_id = Some(seed.k1);
    app.dialog_input.priority_selection.set(Some(3));
    app.handle_set_card_priority_popup(KeyCode::Enter);

    let refetch = refetch_ops(&ops);
    assert!(refetch.is_empty(), "got {refetch:?}");
    assert!(app.ui_state.banner.is_some());
}

#[tokio::test]
async fn test_set_column_default_status_refetches_the_column_tiers_not_the_card_list() {
    let mut app = App::test_default();
    let seed = seed_board_two_columns_two_cards_one_sprint(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    select_column(&mut app, seed.board, seed.c1);
    app.dialog_input.default_status_selection.set(Some(0));
    app.handle_set_column_default_status_popup(KeyCode::Enter);

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
    assert!(
        !has_op_with_id(&refetch, "list_cards_by_column", seed.c2),
        "got {refetch:?}"
    );
    assert!(!has_op(&refetch, "list_all_cards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");
}

#[tokio::test]
async fn test_set_multiple_cards_priority_refetches_the_card_tiers_once_for_the_whole_batch() {
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
    app.multi_select.selected_cards.insert(ids[0]);
    app.multi_select.selected_cards.insert(ids[1]);
    app.dialog_input.priority_selection.set(Some(2));
    app.handle_set_multiple_cards_priority_popup(KeyCode::Enter);

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert_eq!(
        refetch
            .iter()
            .filter(|op| op.method == "list_cards_by_column")
            .count(),
        1,
        "got {refetch:?}"
    );
    assert!(
        has_op_with_id(&refetch, "list_cards_by_column", c1.id),
        "got {refetch:?}"
    );
    assert!(!has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_columns"), "got {refetch:?}");
}

#[tokio::test]
async fn test_order_cards_popup_refetches_the_board_list_not_the_card_tier() {
    let mut app = App::test_default();
    let seed = seed_board_two_columns_two_cards_one_sprint(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.selection.active_sprint_id = None;
    app.filter.sort_field_selection.set(Some(0));
    app.handle_order_cards_popup(KeyCode::Enter);

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(
        has_op_with_id(&refetch, "list_columns_by_board", seed.board),
        "got {refetch:?}"
    );
    assert!(!has_op(&refetch, "list_all_cards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_columns"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_cards_by_column"), "got {refetch:?}");
}

#[tokio::test]
async fn test_assign_card_to_sprint_refetches_the_card_tiers_not_the_sprint_list() {
    let mut app = App::test_default();
    let seed = seed_board_two_columns_two_cards_one_sprint(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.selection.active_card_id = Some(seed.k1);
    let sprints = app
        .model
        .board_sprints_state(seed.board)
        .loaded()
        .copied()
        .unwrap_or(&[])
        .to_vec();
    let board = app
        .model
        .boards_state()
        .loaded_or_empty()
        .iter()
        .find(|b| b.id == seed.board)
        .cloned()
        .expect("board loaded");
    app.dialog_input
        .assign_sprint_picker
        .reset_for_card_assignment(Some(seed.sprint_a), &sprints, &board, chrono::Utc::now());

    app.handle_assign_card_to_sprint_popup(KeyCode::Enter);

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(has_op(&refetch, "list_cards_by_column"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");
    assert!(
        !has_op(&refetch, "list_sprints_by_board"),
        "got {refetch:?}"
    );
}

#[tokio::test]
async fn test_assign_card_to_sprint_failure_issues_no_refetch_read() {
    let mut app = App::test_default();
    let seed = seed_board_two_columns_two_cards_one_sprint(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.selection.active_card_id = Some(seed.k1);
    let sprints = app
        .model
        .board_sprints_state(seed.board)
        .loaded()
        .copied()
        .unwrap_or(&[])
        .to_vec();
    let board = app
        .model
        .boards_state()
        .loaded_or_empty()
        .iter()
        .find(|b| b.id == seed.board)
        .cloned()
        .expect("board loaded");
    app.dialog_input
        .assign_sprint_picker
        .reset_for_card_assignment(Some(seed.sprint_a), &sprints, &board, chrono::Utc::now());

    app.ctx.data_store().delete_sprint(seed.sprint_a).unwrap();

    app.handle_assign_card_to_sprint_popup(KeyCode::Enter);

    let refetch = refetch_ops(&ops);
    assert!(refetch.is_empty(), "got {refetch:?}");
    assert!(app.ui_state.banner.is_some());
}

#[tokio::test]
async fn test_assign_multiple_cards_to_sprint_refetches_the_card_tiers_not_the_sprint_list() {
    let mut app = App::test_default();
    let seed = seed_board_two_columns_two_cards_one_sprint(&mut app);
    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(seed.board);
    app.multi_select.selected_cards.insert(seed.k1);
    app.multi_select.selected_cards.insert(seed.k2);
    let sprints = app
        .model
        .board_sprints_state(seed.board)
        .loaded()
        .copied()
        .unwrap_or(&[])
        .to_vec();
    let board = app
        .model
        .boards_state()
        .loaded_or_empty()
        .iter()
        .find(|b| b.id == seed.board)
        .cloned()
        .expect("board loaded");
    app.dialog_input
        .assign_sprint_picker
        .reset_for_card_assignment(Some(seed.sprint_a), &sprints, &board, chrono::Utc::now());

    app.handle_assign_multiple_cards_to_sprint_popup(KeyCode::Enter);

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
    assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");
    assert!(
        !has_op(&refetch, "list_sprints_by_board"),
        "got {refetch:?}"
    );
}

#[tokio::test]
async fn test_carry_over_sprint_refetches_the_card_tiers_not_the_sprint_list() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let c1 = app
        .ctx
        .create_column(board.id, "C1".to_string(), Some(0))
        .unwrap();
    let source = app.ctx.create_sprint(board.id, None, None).unwrap();
    app.ctx.activate_sprint(source.id, Some(14)).unwrap();
    app.ctx.complete_sprint(source.id).unwrap();
    let target = app.ctx.create_sprint(board.id, None, None).unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            c1.id,
            "K1".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.assign_card_to_sprint(card.id, source.id).unwrap();

    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(board.id);
    app.dialog_input.carry_over_source_sprint_id = Some(source.id);
    app.dialog_input.carry_over_sprint_selection.set(Some(0));
    app.handle_carry_over_sprint_popup(KeyCode::Enter);

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(has_op(&refetch, "list_cards_by_column"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");

    let state = app.model.column_cards_state(c1.id);
    let cards = state
        .loaded()
        .expect("card tier must be loaded after the carry-over refetch");
    let moved = cards
        .iter()
        .find(|c| c.id == card.id)
        .expect("the seeded card must still be present");
    assert_eq!(
        moved.sprint_id,
        Some(target.id),
        "the card must have been carried over onto the target sprint"
    );
}

#[tokio::test]
async fn test_relationship_popup_add_edge_refetches_the_graph_and_the_card_tiers() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let c1 = app
        .ctx
        .create_column(board.id, "C1".to_string(), Some(0))
        .unwrap();
    let a = app
        .ctx
        .create_card(
            board.id,
            c1.id,
            "A".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let b = app
        .ctx
        .create_card(
            board.id,
            c1.id,
            "B".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();

    let ops = prime(&mut app).await;

    app.selection.active_card_id = Some(a.id);
    app.relationship.card_ids = vec![b.id];
    app.relationship.selected.clear();
    app.relationship.selection.set(Some(0));
    app.mode = AppMode::Dialog(DialogMode::ManageChildren);

    app.handle_manage_children_popup(KeyCode::Enter);

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(has_op(&refetch, "get_graph"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_columns"), "got {refetch:?}");
}

#[tokio::test]
async fn test_relationship_popup_remove_edge_refetches_the_graph_and_the_card_tiers() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let c1 = app
        .ctx
        .create_column(board.id, "C1".to_string(), Some(0))
        .unwrap();
    let a = app
        .ctx
        .create_card(
            board.id,
            c1.id,
            "A".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let b = app
        .ctx
        .create_card(
            board.id,
            c1.id,
            "B".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.attach_child(b.id, a.id).unwrap();

    let ops = prime(&mut app).await;

    app.selection.active_card_id = Some(a.id);
    app.relationship.card_ids = vec![b.id];
    app.relationship.selected = std::collections::HashSet::from([b.id]);
    app.relationship.selection.set(Some(0));
    app.mode = AppMode::Dialog(DialogMode::ManageParents);

    app.handle_manage_parents_popup(KeyCode::Enter);

    let refetch = refetch_ops(&ops);
    assert!(!has_op(&refetch, "snapshot"), "got {refetch:?}");
    assert!(has_op(&refetch, "get_graph"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_boards"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");
    assert!(!has_op(&refetch, "list_all_columns"), "got {refetch:?}");
}

#[tokio::test]
async fn test_sprint_detail_move_card_refetches_the_card_tiers_not_the_column_list() {
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
    let sprint = app.ctx.create_sprint(board.id, None, None).unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            c1.id,
            "K1".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.assign_card_to_sprint(card.id, sprint.id).unwrap();

    let snap = kanban_domain::Snapshot {
        archived_boards: Vec::new(),
        boards: app.ctx.data_store().list_boards().unwrap(),
        columns: app.ctx.data_store().list_all_columns().unwrap(),
        cards: app.ctx.data_store().list_all_cards().unwrap(),
        archived_cards: app.ctx.data_store().list_archived_cards().unwrap(),
        sprints: app.ctx.data_store().list_all_sprints().unwrap(),
        graph: app.ctx.data_store().get_graph().unwrap(),
        prefixes: Vec::new(),
    };
    app.load_snapshot(snap);
    app.populate_sprint_task_lists(sprint.id);
    app.sprint_view.panel = kanban_tui::app::sprint_view::SprintTaskPanel::Uncompleted;
    app.sprint_view
        .uncompleted_component
        .update_cards(vec![card.id]);
    app.sprint_view
        .uncompleted_component
        .set_selected_index(Some(0));

    let ops = prime(&mut app).await;

    app.selection.active_board_id = Some(board.id);
    app.selection.active_sprint_id = Some(sprint.id);
    app.mode = AppMode::SprintDetail;

    app.handle_sprint_detail_key(KeyCode::Char('L'));

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
    assert!(!has_op(&refetch, "list_all_sprints"), "got {refetch:?}");
}

#[test]
fn test_no_sprint_dialog_or_popup_handler_still_reloads_wholesale() {
    let sprint_handlers = include_str!("../src/handlers/sprint_handlers.rs");
    let dialog_handlers = include_str!("../src/handlers/dialog_handlers.rs");
    let popup_handlers = include_str!("../src/handlers/popup_handlers.rs");
    let detail_view_handlers = include_str!("../src/handlers/detail_view_handlers.rs");

    fn production_region(src: &str) -> &str {
        let boundary = src.find("#[cfg(test)]").unwrap_or(src.len());
        &src[..boundary]
    }

    for (name, src) in [
        ("sprint_handlers.rs", sprint_handlers),
        ("dialog_handlers.rs", dialog_handlers),
        ("popup_handlers.rs", popup_handlers),
        ("detail_view_handlers.rs", detail_view_handlers),
    ] {
        let region = production_region(src);
        assert_eq!(
            region.matches("reload_model()").count(),
            0,
            "{name} still reloads wholesale"
        );
        assert!(
            !region.contains("EntityIds") && !region.contains("invalidation_from_inverse"),
            "{name} introduced a hand-rolled invalidation instead of using resolve_after_command"
        );
    }
}
