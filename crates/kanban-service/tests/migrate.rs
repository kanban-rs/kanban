use kanban_core::AppConfig;
use kanban_domain::KanbanResult;
use kanban_persistence::StoreRegistry;
use kanban_service::{KanbanContext, StoreManager};

fn manager() -> StoreManager {
    let mut registry = StoreRegistry::new();
    registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    StoreManager::new(registry, kanban_backend::KanbanBackendRegistry::new())
}

async fn open_context(locator: &str, config: AppConfig) -> KanbanResult<KanbanContext> {
    let mut config = config;
    let mut stores = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    stores.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    stores.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    let sm = kanban_service::StoreManager::new(stores, backends);
    sm.sync_backend_with_file(locator, &mut config);
    let backend = sm.make_backend(locator, &config).await?;
    KanbanContext::open(backend, config).await
}

fn now() -> &'static str {
    "2024-01-01T00:00:00Z"
}

fn write_json(dir: &std::path::Path, name: &str, data: serde_json::Value) -> String {
    let path = dir.join(name);
    std::fs::write(&path, serde_json::to_string_pretty(&data).unwrap()).unwrap();
    path.to_str().unwrap().to_string()
}

fn create_test_json(dir: &std::path::Path, name: &str) -> String {
    write_json(
        dir,
        name,
        serde_json::json!({
            "boards": [],
            "columns": [],
            "cards": [],
            "archived_cards": [],
            "sprints": [],
            "graph": { "cards": { "edges": [] } }
        }),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_store_json_to_json_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let from = create_test_json(dir.path(), "source.json");
    let to = dir.path().join("target.json");
    let to_str = to.to_str().unwrap();

    manager()
        .migrate_store("json", &from, "json", to_str)
        .await
        .unwrap();
    assert!(to.exists());
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_store_json_to_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let from = create_test_json(dir.path(), "source.json");
    let to = dir.path().join("target.sqlite");
    let to_str = to.to_str().unwrap();

    manager()
        .migrate_store("json", &from, "sqlite", to_str)
        .await
        .unwrap();
    assert!(to.exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_store_fails_if_target_exists() {
    let dir = tempfile::tempdir().unwrap();
    let from = create_test_json(dir.path(), "source.json");
    let to = create_test_json(dir.path(), "target.json");

    let err = manager()
        .migrate_store("json", &from, "json", &to)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("already exists"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_store_fails_if_source_missing() {
    let dir = tempfile::tempdir().unwrap();
    let from = dir.path().join("nonexistent.json");
    let to = dir.path().join("target.json");

    let err = manager()
        .migrate_store("json", from.to_str().unwrap(), "json", to.to_str().unwrap())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not found"));
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_store_repairs_dangling_sprint_id() {
    let dir = tempfile::tempdir().unwrap();
    let board_id = uuid::Uuid::new_v4().to_string();
    let col_id = uuid::Uuid::new_v4().to_string();
    let card_id = uuid::Uuid::new_v4().to_string();
    let ghost_sprint_id = uuid::Uuid::new_v4().to_string();

    let from = write_json(
        dir.path(),
        "source.json",
        serde_json::json!({
            "boards": [{ "id": board_id, "name": "B",
                "task_sort_field": "Default", "task_sort_order": "Ascending",
                "sprint_name_used_count": 0, "next_sprint_number": 1,
                "task_list_view": "Flat", "prefix_counters": {}, "sprint_counters": {},
                "created_at": now(), "updated_at": now() }],
            "columns": [{ "id": col_id, "board_id": board_id, "name": "TODO",
                "position": 0, "created_at": now(), "updated_at": now() }],
            "sprints": [],
            "cards": [{ "id": card_id, "column_id": col_id, "title": "Orphaned",
                "priority": "Medium", "status": "Todo", "position": 0, "card_number": 1,
                "sprint_id": ghost_sprint_id,
                "sprint_logs": [], "created_at": now(), "updated_at": now() }],
            "archived_cards": [],
            "graph": { "cards": { "edges": [] } }
        }),
    );
    let to = dir.path().join("out.sqlite");

    manager()
        .migrate_store("json", &from, "sqlite", to.to_str().unwrap())
        .await
        .unwrap();

    let ctx = open_context(to.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    let cards = ctx.cards().unwrap();
    assert_eq!(cards.len(), 1, "card should be present");
    assert!(
        cards[0].sprint_id.is_none(),
        "dangling sprint_id should be nulled out"
    );
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_store_repairs_orphaned_column_id() {
    let dir = tempfile::tempdir().unwrap();
    let board_id = uuid::Uuid::new_v4().to_string();
    let valid_col_id = uuid::Uuid::new_v4().to_string();
    let ghost_col_id = uuid::Uuid::new_v4().to_string();
    let card_id = uuid::Uuid::new_v4().to_string();

    let from = write_json(
        dir.path(),
        "source.json",
        serde_json::json!({
            "boards": [{ "id": board_id, "name": "B",
                "task_sort_field": "Default", "task_sort_order": "Ascending",
                "sprint_name_used_count": 0, "next_sprint_number": 1,
                "task_list_view": "Flat", "prefix_counters": {}, "sprint_counters": {},
                "created_at": now(), "updated_at": now() }],
            "columns": [{ "id": valid_col_id, "board_id": board_id, "name": "TODO",
                "position": 0, "created_at": now(), "updated_at": now() }],
            "sprints": [],
            "cards": [{ "id": card_id, "column_id": ghost_col_id, "title": "Orphaned",
                "priority": "Medium", "status": "Todo", "position": 0, "card_number": 1,
                "sprint_logs": [], "created_at": now(), "updated_at": now() }],
            "archived_cards": [],
            "graph": { "cards": { "edges": [] } }
        }),
    );
    let to = dir.path().join("out.sqlite");

    manager()
        .migrate_store("json", &from, "sqlite", to.to_str().unwrap())
        .await
        .unwrap();

    let ctx = open_context(to.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    let cards = ctx.cards().unwrap();
    assert_eq!(cards.len(), 1, "card should be present");
    let expected_col_id = uuid::Uuid::parse_str(&valid_col_id).unwrap();
    assert_eq!(
        cards[0].column_id, expected_col_id,
        "orphaned card should be moved to the first valid column"
    );
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_store_cleans_up_destination_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    let ghost_col_id = uuid::Uuid::new_v4().to_string();
    let card_id = uuid::Uuid::new_v4().to_string();

    // Write JSON that is valid JSON but would produce a non-repairable save error
    // by having a card whose column_id references nothing and there are NO valid columns
    // (so the repair fallback has nothing to fall back to — save will fail)
    let from = write_json(
        dir.path(),
        "source.json",
        serde_json::json!({
            "boards": [],
            "columns": [],
            "sprints": [],
            "cards": [{ "id": card_id, "column_id": ghost_col_id, "title": "T",
                "priority": "Medium", "status": "Todo", "position": 0, "card_number": 1,
                "sprint_logs": [], "created_at": now(), "updated_at": now() }],
            "archived_cards": [],
            "graph": { "cards": { "edges": [] } }
        }),
    );
    let to = dir.path().join("out.sqlite");

    let result = manager()
        .migrate_store("json", &from, "sqlite", to.to_str().unwrap())
        .await;

    assert!(result.is_err(), "migration should fail");
    assert!(
        !to.exists(),
        "destination file should be cleaned up after failure"
    );
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_store_repairs_dangling_completion_column_id() {
    let dir = tempfile::tempdir().unwrap();
    let board_id = uuid::Uuid::new_v4().to_string();
    let col_id = uuid::Uuid::new_v4().to_string();
    let ghost_col_id = uuid::Uuid::new_v4().to_string();

    // A proper V12 envelope: the source load must NOT run the V11->V12
    // backfill (which would silently rewrite the hand-edited list), so the
    // dangling id genuinely reaches the repair seam.
    let from = write_json(
        dir.path(),
        "source.json",
        serde_json::json!({
            "version": 12,
            "metadata": {
                "instance_id": uuid::Uuid::new_v4().to_string(),
                "saved_at": now()
            },
            "data": {
                "boards": [{ "id": board_id, "name": "B",
                    "task_sort_field": "Default", "task_sort_order": "Ascending",
                    "sprint_name_used_count": 0, "next_sprint_number": 1,
                    "task_list_view": "Flat", "prefix_counters": {}, "sprint_counters": {},
                    "completion_column_ids": [ghost_col_id, col_id],
                    "created_at": now(), "updated_at": now() }],
                "columns": [{ "id": col_id, "board_id": board_id, "name": "Done",
                    "position": 0, "created_at": now(), "updated_at": now() }],
                "sprints": [],
                "cards": [],
                "archived_cards": [],
                "graph": {
                    "spawns": { "edges": [] },
                    "blocks": { "edges": [] },
                    "relates": { "edges": [] }
                }
            }
        }),
    );
    let to = dir.path().join("out.sqlite");

    manager()
        .migrate_store("json", &from, "sqlite", to.to_str().unwrap())
        .await
        .unwrap();

    let ctx = open_context(to.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    let board = ctx.boards().unwrap().remove(0);
    assert_eq!(
        board.completion_column_ids,
        vec![col_id.parse::<uuid::Uuid>().unwrap()],
        "a dangling completion id must be pruned so the SQLite FK accepts the import; live ids keep their order"
    );
}

// ─── KAN-1105: cross-format moves through the store adapter ──────────────────

use kanban_backend::KanbanBackend;
use kanban_domain::{Archived, ArchivedCard, Board, Card, Column, Sprint};
use std::sync::Arc;
use uuid::Uuid;

fn full_manager() -> StoreManager {
    let mut stores = StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    stores.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    stores.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    StoreManager::new(stores, backends)
}

async fn open_backend(locator: &str) -> Arc<dyn KanbanBackend> {
    let mut config = AppConfig::default();
    let sm = full_manager();
    sm.sync_backend_with_file(locator, &mut config);
    sm.make_backend(locator, &config).await.unwrap()
}

struct Graph {
    live_board: Uuid,
    live_column: Uuid,
    blocker: Uuid,
    blocked: Uuid,
    archived_card: Uuid,
    live_sprint: Uuid,
    archived_board: Uuid,
    archived_board_column: Uuid,
    archived_board_card: Uuid,
    archived_board_sprint: Uuid,
}

/// A live board with a column, two linked cards, a sprint and an archived card,
/// PLUS a separately archived board carrying its own whole subtree. Anything
/// less cannot catch a migration that drops archived subtrees.
async fn seed_graph(locator: &str) -> Graph {
    let backend = open_backend(locator).await;

    let mut live = Board::new("Live", None::<String>);
    let live_column = Column::new(live.id, "Todo", 0);
    let blocker = Card::new(&mut live, live_column.id, "Blocker", 0);
    let blocked = Card::new(&mut live, live_column.id, "Blocked", 1);
    let archived_card = Card::new(&mut live, live_column.id, "Archived", 2);
    let live_sprint = Sprint::new(live.id, 1, None, None::<String>);

    let mut arch = Board::new("Archived board", None::<String>);
    let arch_column = Column::new(arch.id, "Done", 0);
    let arch_card = Card::new(&mut arch, arch_column.id, "On archived board", 0);
    let arch_sprint = Sprint::new(arch.id, 1, None, None::<String>);

    let g = Graph {
        live_board: live.id,
        live_column: live_column.id,
        blocker: blocker.id,
        blocked: blocked.id,
        archived_card: archived_card.id,
        live_sprint: live_sprint.id,
        archived_board: arch.id,
        archived_board_column: arch_column.id,
        archived_board_card: arch_card.id,
        archived_board_sprint: arch_sprint.id,
    };

    backend.upsert_board(live).unwrap();
    backend.upsert_column(live_column).unwrap();
    backend.upsert_card(blocker).unwrap();
    backend.upsert_card(blocked).unwrap();
    backend.upsert_card(archived_card).unwrap();
    backend.upsert_sprint(live_sprint).unwrap();
    backend
        .insert_archived_card(ArchivedCard::new(g.archived_card, g.live_board))
        .unwrap();

    backend.upsert_board(arch).unwrap();
    backend.upsert_column(arch_column).unwrap();
    backend.upsert_card(arch_card).unwrap();
    backend.upsert_sprint(arch_sprint).unwrap();
    backend
        .insert_archived_board(Archived::now(g.archived_board))
        .unwrap();

    backend
        .modify_graph(Box::new({
            let (a, b) = (g.blocker, g.blocked);
            move |gr| gr.set_block(a, b)
        }))
        .unwrap();

    backend.flush().await.unwrap();
    g
}

/// Reloads `locator` from disk and asserts every seeded entity survived. Uses
/// the unfiltered `get_card`, because an archived card is absent from the
/// live-only listings under the marker model.
async fn assert_graph_survived(locator: &str, g: &Graph, ctx: &str) {
    let backend = open_backend(locator).await;

    assert!(
        backend.get_board(g.live_board).unwrap().is_some(),
        "{ctx}: live board"
    );
    assert!(
        backend.get_board(g.archived_board).unwrap().is_some(),
        "{ctx}: archived board head — absent from list_boards, so a migration \
         that reads only live boards drops this whole subtree"
    );
    assert!(
        backend.get_column(g.live_column).unwrap().is_some(),
        "{ctx}: live column"
    );
    assert!(
        backend
            .get_column(g.archived_board_column)
            .unwrap()
            .is_some(),
        "{ctx}: archived board's column"
    );
    assert!(
        backend.get_card(g.blocker).unwrap().is_some(),
        "{ctx}: blocker card"
    );
    assert!(
        backend.get_card(g.blocked).unwrap().is_some(),
        "{ctx}: blocked card"
    );
    assert!(
        backend.get_card(g.archived_card).unwrap().is_some(),
        "{ctx}: archived card's live row"
    );
    assert!(
        backend.get_card(g.archived_board_card).unwrap().is_some(),
        "{ctx}: archived board's card"
    );
    assert!(
        backend.get_sprint(g.live_sprint).unwrap().is_some(),
        "{ctx}: live sprint"
    );
    assert!(
        backend
            .get_sprint(g.archived_board_sprint)
            .unwrap()
            .is_some(),
        "{ctx}: archived board's sprint"
    );
    assert!(
        backend
            .get_archived_card(g.archived_card)
            .unwrap()
            .is_some(),
        "{ctx}: archived-card marker"
    );
    assert!(
        backend
            .get_archived_board(g.archived_board)
            .unwrap()
            .is_some(),
        "{ctx}: archived-board marker"
    );
    assert_eq!(
        backend.get_graph().unwrap().blockers(g.blocked),
        vec![g.blocker],
        "{ctx}: dependency edge, which lives in the workspace-global graph"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_sqlite_to_json_preserves_full_archival_graph() {
    let dir = tempfile::tempdir().unwrap();
    let from = dir.path().join("source.sqlite");
    let to = dir.path().join("target.json");
    let (from, to) = (from.to_str().unwrap(), to.to_str().unwrap());

    let g = seed_graph(from).await;
    full_manager()
        .migrate_store("sqlite", from, "json", to)
        .await
        .unwrap();

    assert_graph_survived(to, &g, "sqlite -> json").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_json_to_sqlite_preserves_full_archival_graph() {
    let dir = tempfile::tempdir().unwrap();
    let from = dir.path().join("source.json");
    let to = dir.path().join("target.sqlite");
    let (from, to) = (from.to_str().unwrap(), to.to_str().unwrap());

    let g = seed_graph(from).await;
    full_manager()
        .migrate_store("json", from, "sqlite", to)
        .await
        .unwrap();

    assert_graph_survived(to, &g, "json -> sqlite").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_sqlite_to_sqlite_preserves_full_archival_graph() {
    let dir = tempfile::tempdir().unwrap();
    let from = dir.path().join("source.sqlite");
    let to = dir.path().join("target.sqlite");
    let (from, to) = (from.to_str().unwrap(), to.to_str().unwrap());

    let g = seed_graph(from).await;
    full_manager()
        .migrate_store("sqlite", from, "sqlite", to)
        .await
        .unwrap();

    // This leg reads atomically straight into transactional writes, with no
    // JSON serialisation anywhere. The routing decision that guarantees that is
    // asserted in store_manager's unit tests; this proves the data survives it.
    assert_graph_survived(to, &g, "sqlite -> sqlite").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_json_to_sqlite_writes_in_one_transaction() {
    // A column whose board_id names no board is a foreign-key violation SQLite
    // rejects, and FK repair does not rewrite column ownership, so the write
    // fails partway. The destination must not survive half-written.
    let dir = tempfile::tempdir().unwrap();
    let good_board = Uuid::new_v4();
    let from = write_json(
        dir.path(),
        "source.json",
        serde_json::json!({
            "boards": [{
                "id": good_board, "name": "Live", "position": 0,
                "created_at": now(), "updated_at": now()
            }],
            "columns": [
                { "id": Uuid::new_v4(), "board_id": good_board, "name": "Todo",
                  "position": 0, "created_at": now(), "updated_at": now() },
                { "id": Uuid::new_v4(), "board_id": Uuid::new_v4(), "name": "Orphan",
                  "position": 1, "created_at": now(), "updated_at": now() }
            ],
            "cards": [], "archived_cards": [], "sprints": [],
            "graph": { "cards": { "edges": [] } }
        }),
    );
    let to = dir.path().join("target.sqlite");
    let to_str = to.to_str().unwrap();

    let result = full_manager()
        .migrate_store("json", &from, "sqlite", to_str)
        .await;

    assert!(
        result.is_err(),
        "a foreign-key violation partway through the write must surface"
    );
    if to.exists() {
        let backend = open_backend(to_str).await;
        assert!(
            backend.list_boards().unwrap().is_empty(),
            "the transaction must roll back entirely: the board written before \
             the failing column must not survive"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_preserves_a_card_assigned_to_a_sprint() {
    // cards.sprint_id carries a foreign key to sprints(id), so a card that
    // belongs to a sprint constrains the order the destination is written in.
    let dir = tempfile::tempdir().unwrap();
    let from = dir.path().join("source.json");
    let to = dir.path().join("target.sqlite");
    let (from, to) = (from.to_str().unwrap(), to.to_str().unwrap());

    let backend = open_backend(from).await;
    let mut board = Board::new("B", None::<String>);
    let column = Column::new(board.id, "Todo", 0);
    let sprint = Sprint::new(board.id, 1, None, None::<String>);
    let mut card = Card::new(&mut board, column.id, "In a sprint", 0);
    card.sprint_id = Some(sprint.id);
    let (card_id, sprint_id) = (card.id, sprint.id);

    backend.upsert_board(board).unwrap();
    backend.upsert_column(column).unwrap();
    backend.upsert_sprint(sprint).unwrap();
    backend.upsert_card(card).unwrap();
    backend.flush().await.unwrap();

    full_manager()
        .migrate_store("json", from, "sqlite", to)
        .await
        .expect("a card assigned to a sprint must migrate");

    let dest = open_backend(to).await;
    assert_eq!(
        dest.get_card(card_id).unwrap().unwrap().sprint_id,
        Some(sprint_id),
        "the card must still belong to its sprint"
    );
}
