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

    let live = Board::new("Live", None::<String>);
    let live_column = Column::new(live.id, "Todo", 0);
    let blocker = Card::new(live.id, live_column.id, "Blocker", 0);
    let blocked = Card::new(live.id, live_column.id, "Blocked", 1);
    let archived_card = Card::new(live.id, live_column.id, "Archived", 2);
    let live_sprint = Sprint::new(live.id, 1, None, None::<String>);

    let arch = Board::new("Archived board", None::<String>);
    let arch_column = Column::new(arch.id, "Done", 0);
    let arch_card = Card::new(arch.id, arch_column.id, "On archived board", 0);
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
    let board = Board::new("B", None::<String>);
    let column = Column::new(board.id, "Todo", 0);
    let sprint = Sprint::new(board.id, 1, None, None::<String>);
    let mut card = Card::new(board.id, column.id, "In a sprint", 0);
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

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_preserves_a_card_whose_column_is_gone() {
    // cards carry no foreign key on column_id, so a card can outlive its column.
    // The whole-store read this replaced was flat and returned such a row
    // regardless; a read that walks boards -> columns -> cards can only reach a
    // card through a column, and would drop it silently.
    let dir = tempfile::tempdir().unwrap();
    let from = dir.path().join("source.sqlite");
    let to = dir.path().join("target.json");
    let (from, to) = (from.to_str().unwrap(), to.to_str().unwrap());

    let backend = open_backend(from).await;
    let board = Board::new("B", None::<String>);
    let survivor = Column::new(board.id, "Survivor", 0);
    let doomed = Column::new(board.id, "Doomed", 1);
    let card = Card::new(board.id, doomed.id, "Outlives its column", 0);
    let (doomed_id, card_id) = (doomed.id, card.id);

    backend.upsert_board(board).unwrap();
    backend.upsert_column(survivor).unwrap();
    backend.upsert_column(doomed).unwrap();
    backend.upsert_card(card).unwrap();
    backend.delete_column(doomed_id).unwrap();
    backend.flush().await.unwrap();

    assert!(
        backend.get_card(card_id).unwrap().is_some(),
        "precondition: the card must still be in the source after its column went"
    );

    full_manager()
        .migrate_store("sqlite", from, "json", to)
        .await
        .unwrap();

    let dest = open_backend(to).await;
    assert!(
        dest.get_card(card_id).unwrap().is_some(),
        "a card whose column was deleted must survive the migration; FK repair \
         re-homes it, but only if the read carried it across at all"
    );
}

/// The prefix rows carry every card and sprint number in the workspace. A
/// transfer that drops them hands the destination a workspace whose namespaces
/// all restart at 1, so the next card minted re-uses an identifier that is
/// already on a card sitting right beside it.
///
/// Both directions had to be checked. `read_full_snapshot` and
/// `write_full_snapshot` failed independently, so fixing one still left the
/// legs that leaned on the other silently lossy. These four cases are the
/// whole matrix.
mod prefix_transfer {
    use super::*;

    /// A full current-version envelope rather than the bare data object the
    /// other fixtures use: a versionless file runs the whole migration chain,
    /// which rebuilds `prefixes` and would mask what this is testing.
    ///
    /// The version is read from `MAX` rather than written as a literal. Pinned
    /// to a number, the next format bump would quietly turn this into a
    /// migration test and stop exercising the transfer it is here for.
    fn source_with_counters(dir: &std::path::Path) -> String {
        write_json(
            dir,
            "source.json",
            serde_json::json!({
                "version": kanban_persistence::FormatVersion::MAX.as_u32(),
                "metadata": {
                    "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                    "saved_at": "2024-01-01T00:00:00Z"
                },
                "data": {
                    "boards": [],
                    "columns": [],
                    "cards": [],
                    "archived_cards": [],
                    "archived_boards": [],
                    "sprints": [],
                    "graph": { "blocks": { "edges": [] },
                               "relates": { "edges": [] },
                               "spawns": { "edges": [] } },
                    "prefixes": [
                        { "name": "kan", "card_counter": 1258, "sprint_counter": 22 },
                        { "name": "auth", "card_counter": 4, "sprint_counter": 1 }
                    ]
                }
            }),
        )
    }

    async fn counters_at(path: &str, backend: &str) -> Vec<(String, u32, u32)> {
        use kanban_backend::KanbanBackend;
        let store: std::sync::Arc<dyn KanbanBackend> = match backend {
            "sqlite" => std::sync::Arc::new(
                kanban_persistence_sqlite::SqliteBackend::open(path)
                    .await
                    .unwrap(),
            ),
            _ => {
                let file_store = std::sync::Arc::new(kanban_persistence_json::JsonFileStore::new(
                    std::path::Path::new(path),
                ));
                let store = kanban_persistence_json::JsonDataStore::new(file_store);
                store.reload().await.unwrap();
                std::sync::Arc::new(store)
            }
        };
        let mut rows: Vec<(String, u32, u32)> = store
            .list_prefixes()
            .unwrap()
            .into_iter()
            .map(|p| (p.name, p.card_counter, p.sprint_counter))
            .collect();
        rows.sort();
        rows
    }

    fn expected() -> Vec<(String, u32, u32)> {
        vec![("auth".to_string(), 4, 1), ("kan".to_string(), 1258, 22)]
    }

    async fn assert_leg(from_backend: &str, to_backend: &str, ext: &str) {
        let dir = tempfile::tempdir().unwrap();
        let json_source = source_with_counters(dir.path());

        // A SQLite source has to be produced by a transfer of its own; the
        // fixture format is JSON.
        let (source, source_backend) = if from_backend == "sqlite" {
            let staged = dir.path().join("staged.sqlite");
            let staged_str = staged.to_str().unwrap().to_string();
            manager()
                .migrate_store("json", &json_source, "sqlite", &staged_str)
                .await
                .unwrap();
            (staged_str, "sqlite")
        } else {
            (json_source, "json")
        };

        let to = dir.path().join(format!("target.{ext}"));
        let to_str = to.to_str().unwrap();
        manager()
            .migrate_store(source_backend, &source, to_backend, to_str)
            .await
            .unwrap();

        assert_eq!(
            counters_at(to_str, to_backend).await,
            expected(),
            "{from_backend} -> {to_backend} must carry the prefix counters"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_json_to_sqlite_carries_the_prefix_counters() {
        assert_leg("json", "sqlite", "sqlite").await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_json_to_json_carries_the_prefix_counters() {
        assert_leg("json", "json", "json").await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sqlite_to_json_carries_the_prefix_counters() {
        assert_leg("sqlite", "json", "json").await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sqlite_to_sqlite_carries_the_prefix_counters() {
        assert_leg("sqlite", "sqlite", "sqlite").await;
    }
}
