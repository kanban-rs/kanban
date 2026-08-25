//! Whole-graph coverage for the V14 -> V18 migration chain (prefixes
//! backfill, card-prefix stamping, legacy-counter drop, prefix-row repair).
//!
//! Every other test in this chain fixtures a bare board plus cards, with
//! empty `columns`, `sprints`, and edge arrays, so the chain is proven only
//! to preserve boards and cards. This seeds a NON-TRIVIAL graph -- two
//! columns (one WIP-limited), five cards spread across them, a sprint with
//! two cards bound to it, one archived card (live under the reference-marker
//! model, per `kanban_domain::archived_card`), and one edge of each of the
//! three kinds -- and asserts every one of those survives the full chain
//! with ids, positions, WIP limits, sprint bindings, and edges intact.
//!
//! Modeled on `tests/v15_migration.rs` (the V14 fixture shape) and
//! `tests/v10_migration.rs` (the archived-card reference-marker shape).

use kanban_persistence::{FormatVersion, PersistenceStore};
use kanban_persistence_json::migration::Migrator;
use kanban_persistence_json::JsonFileStore;
use serde_json::{json, Value};
use tempfile::tempdir;

const BOARD: &str = "11111111-1111-1111-1111-111111111111";
const COL_TODO: &str = "22222222-2222-2222-2222-222222222222";
const COL_DOING: &str = "33333333-3333-3333-3333-333333333333";
const SPRINT: &str = "44444444-4444-4444-4444-444444444444";

const CARD_A: &str = "a0000000-0000-0000-0000-000000000001";
const CARD_B: &str = "a0000000-0000-0000-0000-000000000002";
const CARD_C: &str = "a0000000-0000-0000-0000-000000000003";
const CARD_D: &str = "a0000000-0000-0000-0000-000000000004";
const CARD_ARCHIVED: &str = "a0000000-0000-0000-0000-000000000005";

fn card(id: &str, column_id: &str, position: i64, sprint_id: Option<&str>) -> Value {
    json!({
        "id": id,
        "column_id": column_id,
        "board_id": BOARD,
        "title": format!("Card {id}"),
        "description": null,
        "priority": "Medium",
        "status": "Todo",
        "position": position,
        "due_date": null,
        "points": null,
        "card_number": position + 1,
        "sprint_id": sprint_id,
        "sprint_logs": [],
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-01T00:00:00Z",
        "completed_at": null
    })
}

fn v14_whole_graph_fixture() -> Value {
    json!({
        "version": 14,
        "metadata": {
            "instance_id": "00000000-0000-0000-0000-000000000001",
            "saved_at": "2024-01-01T00:00:00Z"
        },
        "data": {
            "boards": [{
                "id": BOARD, "name": "Kanban", "card_prefix": "KAN", "card_counter": 5,
                "description": null, "sprint_prefix": null,
                "task_sort_field": "Position", "task_sort_order": "Ascending",
                "sprint_duration_days": null, "sprint_names": ["Sprint One"],
                "sprint_name_used_count": 1, "sprint_counters": {"kan": 1},
                "next_sprint_number": 2, "active_sprint_id": null,
                "task_list_view": "Flat", "position": 0,
                "created_at": "2024-01-01T00:00:00Z", "updated_at": "2024-01-01T00:00:00Z"
            }],
            "columns": [
                {
                    "id": COL_TODO, "board_id": BOARD, "name": "Todo", "position": 0,
                    "wip_limit": null,
                    "created_at": "2024-01-01T00:00:00Z", "updated_at": "2024-01-01T00:00:00Z"
                },
                {
                    "id": COL_DOING, "board_id": BOARD, "name": "Doing", "position": 1,
                    "wip_limit": 3,
                    "created_at": "2024-01-01T00:00:00Z", "updated_at": "2024-01-01T00:00:00Z"
                }
            ],
            "cards": [
                card(CARD_A, COL_TODO, 0, None),
                card(CARD_B, COL_TODO, 1, Some(SPRINT)),
                card(CARD_C, COL_DOING, 0, Some(SPRINT)),
                card(CARD_D, COL_DOING, 1, None),
                card(CARD_ARCHIVED, COL_DOING, 2, None)
            ],
            "sprints": [{
                "id": SPRINT, "board_id": BOARD, "name": "Sprint One", "number": 1,
                "status": "Active", "card_prefix": null,
                "start_date": null, "end_date": null,
                "created_at": "2024-01-01T00:00:00Z", "updated_at": "2024-01-01T00:00:00Z"
            }],
            "archived_cards": [{
                "entity_id": CARD_ARCHIVED,
                "board_id": BOARD,
                "archived_at": "2024-01-02T00:00:00Z"
            }],
            "graph": {
                "spawns": { "edges": [
                    { "source": CARD_A, "target": CARD_B, "created_at": "2024-01-01T00:00:00Z", "archived_at": null }
                ] },
                "blocks": { "edges": [
                    { "source": CARD_B, "target": CARD_C, "severity": "High", "created_at": "2024-01-01T00:00:00Z", "archived_at": null }
                ] },
                "relates": { "edges": [
                    { "source": CARD_C, "target": CARD_D, "kind": "Duplicates", "created_at": "2024-01-01T00:00:00Z", "archived_at": null }
                ] }
            }
        }
    })
}

fn write_fixture(path: &std::path::Path) {
    std::fs::write(
        path,
        serde_json::to_string_pretty(&v14_whole_graph_fixture()).unwrap(),
    )
    .unwrap();
}

fn read_json(path: &std::path::Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn ids_in(arr: &Value, key: &str) -> Vec<String> {
    arr.as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e.get(key).and_then(Value::as_str).map(str::to_string))
        .collect()
}

fn card_by_id<'a>(env: &'a Value, id: &str) -> &'a Value {
    env["data"]["cards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("card {id} missing after migration"))
}

/// Not a red step: the migrations are believed correct and this is a
/// regression pin over a graph shape no fixture in this chain previously
/// covered. If any assertion below had failed, that would have been a real
/// migration defect worth stopping for, not something to paper over here.
#[tokio::test]
async fn test_migrate_v14_to_v18_preserves_the_whole_entity_graph() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("board.json");
    write_fixture(&path);

    Migrator::migrate(FormatVersion::V14, FormatVersion::MAX, &path)
        .await
        .expect("V14 -> V18 must succeed over a non-trivial graph");

    let after = read_json(&path);
    assert_eq!(after["version"], 18);

    let board_ids = ids_in(&after["data"]["boards"], "id");
    assert_eq!(board_ids, vec![BOARD.to_string()], "the board survives");

    let columns = after["data"]["columns"].as_array().unwrap();
    assert_eq!(columns.len(), 2, "both columns survive");
    let todo = columns.iter().find(|c| c["id"] == COL_TODO).unwrap();
    assert_eq!(todo["wip_limit"], Value::Null);
    let doing = columns.iter().find(|c| c["id"] == COL_DOING).unwrap();
    assert_eq!(doing["wip_limit"], 3, "the WIP limit on Doing survives");

    let card_ids = ids_in(&after["data"]["cards"], "id");
    for id in [CARD_A, CARD_B, CARD_C, CARD_D, CARD_ARCHIVED] {
        assert!(card_ids.contains(&id.to_string()), "card {id} survives");
    }
    assert_eq!(card_by_id(&after, CARD_C)["position"], 0);
    assert_eq!(card_by_id(&after, CARD_D)["position"], 1);

    let sprint_ids = ids_in(&after["data"]["sprints"], "id");
    assert_eq!(sprint_ids, vec![SPRINT.to_string()], "the sprint survives");
    assert_eq!(card_by_id(&after, CARD_B)["sprint_id"], SPRINT);
    assert_eq!(card_by_id(&after, CARD_C)["sprint_id"], SPRINT);
    assert_eq!(card_by_id(&after, CARD_A)["sprint_id"], Value::Null);

    let archived = after["data"]["archived_cards"].as_array().unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0]["entity_id"], CARD_ARCHIVED);
    assert_eq!(archived[0]["board_id"], BOARD);

    let spawns = after["data"]["graph"]["spawns"]["edges"]
        .as_array()
        .unwrap();
    assert_eq!(spawns.len(), 1, "the spawns edge survives");
    assert_eq!(spawns[0]["source"], CARD_A);
    assert_eq!(spawns[0]["target"], CARD_B);

    let blocks = after["data"]["graph"]["blocks"]["edges"]
        .as_array()
        .unwrap();
    assert_eq!(blocks.len(), 1, "the blocks edge survives");
    assert_eq!(blocks[0]["source"], CARD_B);
    assert_eq!(blocks[0]["target"], CARD_C);
    assert_eq!(blocks[0]["severity"], "High");

    let relates = after["data"]["graph"]["relates"]["edges"]
        .as_array()
        .unwrap();
    assert_eq!(relates.len(), 1, "the relates edge survives");
    assert_eq!(relates[0]["source"], CARD_C);
    assert_eq!(relates[0]["target"], CARD_D);
    assert_eq!(relates[0]["kind"], "Duplicates");

    let prefixes = after["data"]["prefixes"].as_array().unwrap();
    assert!(
        prefixes.iter().any(|p| p["name"] == "kan"),
        "the board's namespace was backfilled: {prefixes:#?}"
    );
    for id in [CARD_A, CARD_B, CARD_C, CARD_D, CARD_ARCHIVED] {
        assert_eq!(
            card_by_id(&after, id)["prefix"],
            "KAN",
            "card {id} was stamped with its board's namespace"
        );
    }

    let board = &after["data"]["boards"][0];
    assert!(board.get("card_counter").is_none());
    assert!(board.get("sprint_counters").is_none());
}

/// Mirrors `test_sync_and_async_chains_produce_identical_v15_prefixes_output`
/// in `tests/v15_migration.rs`, but over the whole graph rather than a
/// boards-and-sprints-only fixture: the async orchestrator and the sync
/// `load_sync` entry point must reach byte-identical output on a graph that
/// also carries columns, sprint-bound cards, an archived card, and edges of
/// every kind.
#[tokio::test]
async fn test_sync_and_async_chains_agree_on_the_whole_graph() {
    let dir = tempdir().unwrap();
    let async_path = dir.path().join("async.json");
    let sync_path = dir.path().join("sync.json");
    write_fixture(&async_path);
    write_fixture(&sync_path);

    Migrator::migrate(FormatVersion::V14, FormatVersion::MAX, &async_path)
        .await
        .expect("async V14 -> V18 must succeed");

    let sync_store = JsonFileStore::new(&sync_path);
    let _ = sync_store.load_sync().unwrap().expect("file exists");

    let async_after = read_json(&async_path);
    let sync_after = read_json(&sync_path);

    assert_eq!(
        async_after, sync_after,
        "the async and sync migration chains must produce identical output over the whole graph"
    );
}
