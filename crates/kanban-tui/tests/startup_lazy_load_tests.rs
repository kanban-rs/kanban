mod helpers;

use helpers::{CountingBackend, ReadOp};
use kanban_domain::{KanbanOperations, LoadState};
use kanban_tui::App;

struct SeededBoard {
    id: uuid::Uuid,
}

fn seed_board_with_subtree(app: &mut App, name: &str) -> SeededBoard {
    let board = app.ctx.create_board(name.to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    app.ctx
        .create_card(
            board.id,
            column.id,
            "Card".to_string(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx
        .create_sprint(board.id, None, Some("Sprint".to_string()))
        .unwrap();
    SeededBoard { id: board.id }
}

#[tokio::test]
async fn test_startup_does_not_read_the_whole_store() {
    let mut app = App::test_default();
    let board1 = seed_board_with_subtree(&mut app, "Board 1");
    let _board2 = seed_board_with_subtree(&mut app, "Board 2");

    let (backend, _reads, ops) = CountingBackend::wrap(app.ctx.backend());
    app.ctx.replace_backend(backend);

    app.load_initial_state().await;

    let ops = ops.lock().unwrap().clone();

    assert!(
        ops.iter().any(|op| op.method == "list_boards"),
        "expected a list_boards op, got {ops:?}"
    );
    assert!(
        !ops.iter().any(|op| op.method == "snapshot"),
        "startup must not read the whole workspace via a single snapshot call, got {ops:?}"
    );
    assert!(
        ops.iter().all(|op| {
            if matches!(
                op.method,
                "list_columns_by_board" | "list_sprints_by_board" | "list_cards_by_column"
            ) {
                op.method == "list_cards_by_column" || op.ids == vec![board1.id]
            } else {
                true
            }
        }),
        "expected board-scoped reads to target only the auto-selected board: {ops:?}"
    );
}

#[tokio::test]
async fn test_startup_reads_the_board_list_before_the_auto_selected_boards_columns() {
    let mut app = App::test_default();
    let _board1 = seed_board_with_subtree(&mut app, "Board 1");

    let (backend, _reads, ops) = CountingBackend::wrap(app.ctx.backend());
    app.ctx.replace_backend(backend);

    app.load_initial_state().await;

    let ops = ops.lock().unwrap().clone();
    let boards_pos = ops
        .iter()
        .position(|op: &ReadOp| op.method == "list_boards");
    let columns_pos = ops
        .iter()
        .position(|op| op.method == "list_columns_by_board");
    assert!(
        boards_pos.is_some() && columns_pos.is_some() && boards_pos < columns_pos,
        "expected list_boards strictly before the board-scoped column read, got {ops:?}"
    );
}

#[tokio::test]
async fn test_an_unvisited_boards_tiers_stay_not_loaded_after_startup() {
    let mut app = App::test_default();
    let _board1 = seed_board_with_subtree(&mut app, "Board 1");
    let board2 = seed_board_with_subtree(&mut app, "Board 2");

    app.load_initial_state().await;

    assert!(matches!(
        app.model.board_columns_state(board2.id),
        LoadState::NotLoaded
    ));
    assert!(matches!(
        app.model.board_sprints_state(board2.id),
        LoadState::NotLoaded
    ));
}

#[tokio::test]
async fn test_startup_loads_the_auto_selected_boards_subtree() {
    let mut app = App::test_default();
    let board1 = seed_board_with_subtree(&mut app, "Board 1");

    app.load_initial_state().await;

    assert!(matches!(app.model.columns_state(), LoadState::Loaded(_)));
    assert!(matches!(app.model.sprints_state(), LoadState::Loaded(_)));
    assert_eq!(app.selection.active_board_id, None);
    assert_eq!(app.board_list.get_selected_board_id(), Some(board1.id));
}

#[tokio::test]
async fn test_startup_on_an_empty_store_renders_without_a_board_in_scope() {
    let mut app = App::test_default();

    app.load_initial_state().await;

    assert!(matches!(app.model.boards_state(), LoadState::Loaded(_)));
    assert!(app.board_list.get_selected_board_id().is_none());
}

#[tokio::test]
async fn test_startup_runs_the_sprint_log_migration_before_the_first_fetch() {
    let mut app = App::test_default();
    let _board1 = seed_board_with_subtree(&mut app, "Board 1");

    app.load_initial_state().await;

    assert!(!app.ctx.is_dirty());
}

#[tokio::test]
async fn test_a_failed_initial_read_clears_the_save_file_and_raises_a_banner() {
    let mut app = App::test_default();
    app.persistence.save_file = Some("/tmp/does-not-matter.json".to_string());

    let backend = CountingBackend::wrap_failing(app.ctx.backend(), "list_boards");
    app.ctx.replace_backend(backend);

    app.load_initial_state().await;

    assert_eq!(app.persistence.save_file, None);
    assert!(app.ui_state.banner.is_some());
}
