mod helpers;

use helpers::{CountingBackend, ReadOp};
use kanban_domain::{KanbanOperations, LoadState};
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::AppMode;
use kanban_tui::App;

/// No live board remains, so the only way either archived view can show its
/// entity is via an entry-triggered reload.
#[tokio::test]
async fn test_entering_an_archived_view_after_startup_displays_its_entity() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Card".to_string(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.archive_card(card.id).unwrap();
    app.ctx.archive_board(board.id).unwrap();

    let (backend, _reads, ops) = CountingBackend::wrap(app.ctx.backend());
    app.ctx.replace_backend(backend);

    app.load_initial_state().await;

    let startup_ops = ops.lock().unwrap().clone();
    assert!(
        !startup_ops
            .iter()
            .any(|op| op.method == "list_archived_cards" || op.method == "list_archived_boards"),
        "startup must not read anything archived, got {startup_ops:?}"
    );

    app.focus.active = Focus::Boards;
    app.handle_toggle_archived_cards_view();
    assert_eq!(app.mode, AppMode::ArchivedCardsView);
    let archived_tasks: Vec<_> = app.displayed_cards().iter().map(|c| c.id).collect();
    assert_eq!(
        archived_tasks,
        vec![card.id],
        "expected the archived cards view to display the archived card on entry"
    );

    app.handle_toggle_archived_cards_view();
    assert_eq!(app.mode, AppMode::Normal);

    app.handle_toggle_archived_boards_view();
    assert_eq!(app.mode, AppMode::ArchivedBoardsView);
    let archived_projects: Vec<_> = app.displayed_boards().iter().map(|b| b.id).collect();
    assert_eq!(
        archived_projects,
        vec![board.id],
        "expected the archived boards view to display the archived board on entry"
    );
}

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
async fn test_startup_reads_the_board_list_and_board_scoped_tiers_instead_of_one_bulk_snapshot() {
    let mut app = App::test_default();
    let board1 = seed_board_with_subtree(&mut app, "Board 1");
    let board2 = seed_board_with_subtree(&mut app, "Board 2");

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
    let whole_store_list_reads = ops
        .iter()
        .filter(|op| {
            matches!(
                op.method,
                "list_all_columns" | "list_all_cards" | "list_all_sprints"
            )
        })
        .count();
    assert!(
        whole_store_list_reads <= 5,
        "expected at most 5 whole-store list reads (1 list_all_columns + 1 list_all_cards \
         + 1 list_all_sprints from ViewScope's transitional flat arms, plus 1 list_all_cards \
         + 1 list_all_sprints from migrate_sprint_logs), got {whole_store_list_reads} in {ops:?}"
    );
    assert!(
        !ops.iter()
            .any(|op| op.method == "list_archived_cards" || op.method == "list_archived_boards"),
        "startup must not read anything archived, got {ops:?}"
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
    assert!(
        !ops.iter().any(|o| o.ids.contains(&board2.id)),
        "no startup read may target the unvisited board: {ops:?}"
    );
    assert!(
        matches!(
            app.model.board_columns_state(board2.id),
            LoadState::NotLoaded
        ),
        "the unvisited board's column tier must stay NotLoaded"
    );
    assert!(
        matches!(
            app.model.board_sprints_state(board2.id),
            LoadState::NotLoaded
        ),
        "the unvisited board's sprint tier must stay NotLoaded"
    );
    assert!(
        app.model.board_columns_state(board1.id).is_loaded(),
        "expected the auto-selected board's column tier to be Loaded after startup"
    );
    assert!(
        ops.iter()
            .any(|op| op.method == "list_columns_by_board" && op.ids == vec![board1.id]),
        "expected a list_columns_by_board read scoped to the auto-selected board: {ops:?}"
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
    let board = app.ctx.create_board("Board 1".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let sprint = app
        .ctx
        .create_sprint(board.id, None, Some("Sprint".to_string()))
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Card".to_string(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();

    let backend = app.ctx.backend();
    let mut stale_card = backend.get_card(card.id).unwrap().unwrap();
    stale_card.sprint_id = Some(sprint.id);
    stale_card.sprint_logs.clear();
    backend.upsert_card(stale_card).unwrap();

    app.load_initial_state().await;

    let migrated_log_present = app
        .model
        .cards_state()
        .loaded_or_empty()
        .iter()
        .find(|c| c.id == card.id)
        .map(|c| !c.sprint_logs.is_empty())
        .unwrap_or(false);
    assert!(
        migrated_log_present,
        "load_initial_state's first fetch must serve the migrated sprint log, not the stale pre-migration row"
    );
}

#[tokio::test]
async fn test_startup_marks_the_migration_clean_so_the_conflict_popup_does_not_fire() {
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
    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("expected an error banner");
    assert!(
        banner.message.contains("Failed to read data file"),
        "got: {}",
        banner.message
    );
    assert!(
        banner.message.contains("injected fault: list_boards"),
        "got: {}",
        banner.message
    );
}

#[tokio::test]
async fn test_a_corrupt_data_file_clears_the_save_file_and_raises_a_banner() {
    let mut app = App::test_default();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("corrupt.json");
    std::fs::write(&path, b"not valid json").unwrap();
    let path_str = path.to_str().unwrap().to_string();
    app.persistence.save_file = Some(path_str.clone());

    let store = kanban_persistence_json::JsonFileStore::new(&path_str);
    let backend: std::sync::Arc<dyn kanban_service::KanbanBackend> = std::sync::Arc::new(
        kanban_persistence_json::JsonDataStore::new(std::sync::Arc::new(store)),
    );
    app.ctx.replace_backend(backend);

    app.load_initial_state().await;

    assert_eq!(app.persistence.save_file, None);
    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("a genuinely corrupt data file must surface an error banner too");
    assert!(
        banner.message.contains("Failed to read data file"),
        "got: {}",
        banner.message
    );
}

mod backend_parity {
    use super::*;
    use kanban_backend_memory::InMemoryStore;
    use kanban_domain::{CreateCardOptions, GraphOperations, Model, Snapshot};
    use kanban_persistence_json::{JsonDataStore, JsonFileStore};
    use kanban_persistence_sqlite::SqliteBackend;
    use kanban_service::{test_helpers::contract::assert_card_eq, AppConfig, KanbanContext};
    use kanban_tui::app::ViewScope;
    use std::sync::Arc;
    use uuid::Uuid;

    async fn open_seeded(
        kind: &str,
        dir: &tempfile::TempDir,
        snapshot: &Snapshot,
    ) -> KanbanContext {
        let backend: Arc<dyn kanban_service::KanbanBackend> = match kind {
            "memory" => Arc::new(InMemoryStore::new()),
            "json" => Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(
                dir.path().join("test.json"),
            )))),
            "sqlite" => Arc::new(
                SqliteBackend::open(dir.path().join("test.sqlite3").to_str().unwrap())
                    .await
                    .unwrap(),
            ),
            other => panic!("unknown backend kind {other}"),
        };
        backend.apply_snapshot(snapshot.clone()).unwrap();
        KanbanContext::open(backend, AppConfig::default())
            .await
            .unwrap()
    }

    /// Builds one canonical seed (two boards; board 1 has a column, two
    /// cards linked by a spawns edge, and a sprint) via a throwaway
    /// in-memory context, so every backend under test is seeded from the
    /// exact same `Snapshot` value rather than three independently timed
    /// `create_*` calls that would drift in `created_at`/`updated_at`.
    async fn seed_non_trivial_snapshot() -> (Snapshot, Uuid, Uuid) {
        let backend: Arc<dyn kanban_service::KanbanBackend> = Arc::new(InMemoryStore::new());
        let mut ctx = KanbanContext::open(backend, AppConfig::default())
            .await
            .unwrap();
        let board1 = ctx.create_board("Board 1".to_string(), None).unwrap();
        let board2 = ctx.create_board("Board 2".to_string(), None).unwrap();
        let column = ctx
            .create_column(board1.id, "Todo".to_string(), None)
            .unwrap();
        let card1 = ctx
            .create_card(
                board1.id,
                column.id,
                "Card 1".to_string(),
                CreateCardOptions::default(),
            )
            .unwrap();
        let card2 = ctx
            .create_card(
                board1.id,
                column.id,
                "Card 2".to_string(),
                CreateCardOptions::default(),
            )
            .unwrap();
        ctx.attach_child(card1.id, card2.id).unwrap();
        ctx.create_sprint(board1.id, None, Some("Sprint 1".to_string()))
            .unwrap();
        let snapshot = ctx.snapshot().unwrap();
        (snapshot, board1.id, board2.id)
    }

    async fn seed_empty_board_snapshot() -> (Snapshot, Uuid) {
        let backend: Arc<dyn kanban_service::KanbanBackend> = Arc::new(InMemoryStore::new());
        let mut ctx = KanbanContext::open(backend, AppConfig::default())
            .await
            .unwrap();
        let board = ctx.create_board("Empty Board".to_string(), None).unwrap();
        let snapshot = ctx.snapshot().unwrap();
        (snapshot, board.id)
    }

    fn populate_scope(ctx: &KanbanContext, board_id: Uuid) -> Model {
        let mut model = Model::default();
        let pass1 = ctx.resolve(
            &ViewScope {
                board_list: true,
                ..Default::default()
            },
            &model,
        );
        let _ = model.apply_resolved(pass1);
        let pass2 = ctx.resolve(
            &ViewScope {
                board_list: true,
                board: Some(board_id),
                board_columns: true,
                board_cards: true,
                board_sprints: true,
                graph: true,
                ..Default::default()
            },
            &model,
        );
        let _ = model.apply_resolved(pass2);
        model
    }

    fn sorted_by_id<T: Clone, K: Ord>(items: &[T], key: impl Fn(&T) -> K) -> Vec<T> {
        let mut items = items.to_vec();
        items.sort_by_key(key);
        items
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_startup_scope_reads_the_same_rows_on_every_backend() {
        let (snapshot, board1, _board2) = seed_non_trivial_snapshot().await;
        let kinds = ["memory", "json", "sqlite"];
        let dirs = [
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
        ];

        let mut models = Vec::new();
        for (kind, dir) in kinds.iter().zip(dirs.iter()) {
            let ctx = open_seeded(kind, dir, &snapshot).await;
            models.push((*kind, populate_scope(&ctx, board1)));
        }

        let (baseline_kind, baseline) = &models[0];

        for (kind, model) in &models[1..] {
            let expected_boards = sorted_by_id(baseline.boards_state().loaded().unwrap(), |b| b.id);
            let actual_boards = sorted_by_id(model.boards_state().loaded().unwrap(), |b| b.id);
            assert_eq!(
                actual_boards, expected_boards,
                "boards differ between {baseline_kind} and {kind}"
            );

            let expected_columns =
                sorted_by_id(baseline.columns_state().loaded().unwrap(), |c| c.id);
            let actual_columns = sorted_by_id(model.columns_state().loaded().unwrap(), |c| c.id);
            assert_eq!(
                actual_columns, expected_columns,
                "columns differ between {baseline_kind} and {kind}"
            );

            let mut expected_cards = baseline.cards_state().loaded().unwrap().clone();
            expected_cards.sort_by_key(|c| c.id);
            let mut actual_cards = model.cards_state().loaded().unwrap().clone();
            actual_cards.sort_by_key(|c| c.id);
            assert_eq!(
                expected_cards.len(),
                actual_cards.len(),
                "card count differs between {baseline_kind} and {kind}"
            );
            for (a, b) in expected_cards.iter().zip(actual_cards.iter()) {
                assert_card_eq(a, b);
            }

            let expected_sprints =
                sorted_by_id(baseline.sprints_state().loaded().unwrap(), |s| s.id);
            let actual_sprints = sorted_by_id(model.sprints_state().loaded().unwrap(), |s| s.id);
            assert_eq!(
                actual_sprints, expected_sprints,
                "sprints differ between {baseline_kind} and {kind}"
            );

            assert_eq!(
                baseline.graph_state().loaded(),
                model.graph_state().loaded(),
                "dependency graph differs between {baseline_kind} and {kind}"
            );
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_startup_scope_reports_the_same_variant_for_an_empty_board_on_every_backend() {
        let (snapshot, board_id) = seed_empty_board_snapshot().await;
        let kinds = ["memory", "json", "sqlite"];
        let dirs = [
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
        ];

        for (kind, dir) in kinds.iter().zip(dirs.iter()) {
            let ctx = open_seeded(kind, dir, &snapshot).await;
            let model = populate_scope(&ctx, board_id);

            assert!(
                model.board_columns_state(board_id).is_loaded(),
                "{kind}: an empty board's column tier must resolve to Loaded(vec![]), not NotLoaded/Missing"
            );
            assert_eq!(
                model
                    .board_columns_state(board_id)
                    .loaded()
                    .map(|c| c.len()),
                Some(0),
                "{kind}: expected zero columns"
            );
            assert!(
                model.board_sprints_state(board_id).is_loaded(),
                "{kind}: an empty board's sprint tier must resolve to Loaded(vec![]), not NotLoaded/Missing"
            );
            assert!(
                model.cards_state().is_loaded(),
                "{kind}: the flat card tier must resolve to Loaded(vec![])"
            );
        }
    }
}
