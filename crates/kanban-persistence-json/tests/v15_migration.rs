//! Dedicated coverage for the V14 -> V15 `prefixes` backfill: sync/async
//! chain equality, and an identifier-preservation proof that the backfilled
//! `prefixes` rows agree with `kanban_domain::prefix`'s dynamic resolution
//! for the same board/sprint graph.

use kanban_domain::board::Board;
use kanban_domain::prefix::{effective_prefixes, find_prefix_collisions};
use kanban_domain::sprint::Sprint;
use kanban_persistence::{FormatVersion, PersistenceStore};
use kanban_persistence_json::migration::Migrator;
use kanban_persistence_json::JsonFileStore;
use serde_json::{json, Value};
use tempfile::tempdir;

const BOARD_KAN: &str = "11111111-1111-1111-1111-111111111111";
const BOARD_DEV: &str = "22222222-2222-2222-2222-222222222222";
const BOARD_DEFAULT: &str = "33333333-3333-3333-3333-333333333333";
const SPRINT_AUTH: &str = "44444444-4444-4444-4444-444444444444";

fn read_json(path: &std::path::Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn v14_fixture() -> Value {
    json!({
        "version": 14,
        "metadata": {
            "instance_id": "00000000-0000-0000-0000-000000000001",
            "saved_at": "2024-01-01T00:00:00Z"
        },
        "data": {
            "boards": [
                { "id": BOARD_KAN, "name": "Kanban", "card_prefix": "kan", "card_counter": 12 },
                { "id": BOARD_DEV, "name": "Dev", "card_prefix": "dev", "card_counter": 3 },
                { "id": BOARD_DEFAULT, "name": "Fallback", "card_prefix": null, "card_counter": 1 }
            ],
            "columns": [],
            "cards": [],
            "archived_cards": [],
            "sprints": [
                { "id": SPRINT_AUTH, "board_id": BOARD_KAN, "card_prefix": "auth" }
            ],
            "graph": {
                "spawns": { "edges": [] },
                "blocks": { "edges": [] },
                "relates": { "edges": [] }
            }
        }
    })
}

fn write_fixture(path: &std::path::Path) {
    std::fs::write(path, serde_json::to_string_pretty(&v14_fixture()).unwrap()).unwrap();
}

#[tokio::test]
async fn test_sync_and_async_chains_produce_identical_v15_prefixes_output() {
    let dir = tempdir().unwrap();
    let async_path = dir.path().join("async.json");
    let sync_path = dir.path().join("sync.json");
    write_fixture(&async_path);
    write_fixture(&sync_path);

    Migrator::migrate(FormatVersion::V14, FormatVersion::MAX, &async_path)
        .await
        .expect("async V14 -> V15 must succeed");

    let sync_store = JsonFileStore::new(&sync_path);
    let _ = sync_store.load_sync().unwrap().expect("file exists");

    let async_after = read_json(&async_path);
    let sync_after = read_json(&sync_path);

    assert_eq!(
        async_after, sync_after,
        "the async migration chain and the sync migration chain must produce identical V15 output, including the prefixes array"
    );
    assert_eq!(
        async_after["data"]["prefixes"].as_array().unwrap().len(),
        4,
        "three board rows plus one sprint override row"
    );
}

#[test]
fn test_migrate_v14_to_v15_identifier_preservation() {
    let mut board_kan = Board::new("Kanban", Some("kan"));
    board_kan.id = BOARD_KAN.parse().unwrap();
    let mut board_dev = Board::new("Dev", Some("dev"));
    board_dev.id = BOARD_DEV.parse().unwrap();
    let mut board_default = Board::new("Fallback", None::<String>);
    board_default.id = BOARD_DEFAULT.parse().unwrap();

    let mut sprint_auth = Sprint::new(board_kan.id, 1, None, None::<String>);
    sprint_auth.id = SPRINT_AUTH.parse().unwrap();
    sprint_auth.card_prefix = Some("auth".to_string());

    let boards = vec![board_kan.clone(), board_dev.clone(), board_default.clone()];
    let sprints = vec![sprint_auth.clone()];

    let before = effective_prefixes(&boards, &sprints, "task");
    let before_collisions = find_prefix_collisions(&before);
    assert!(
        before_collisions.is_empty(),
        "fixture must be collision-free before migration"
    );

    let mut env = v14_fixture();
    kanban_persistence_json::migration_test_support::transform_v14_to_v15(&mut env);

    let rows = env["data"]["prefixes"].as_array().unwrap();
    for entry in &before {
        let owner_id = match entry.owner {
            kanban_domain::prefix::PrefixOwner::Board(id) => id.to_string(),
            kanban_domain::prefix::PrefixOwner::Sprint(id) => id.to_string(),
        };
        let row = rows
            .iter()
            .find(|r| r["owner_id"].as_str() == Some(owner_id.as_str()))
            .unwrap_or_else(|| panic!("no backfilled row for owner {owner_id}"));
        assert_eq!(
            row["name"].as_str().unwrap(),
            entry.name.as_str(),
            "the backfilled prefix for owner {owner_id} must equal the dynamically-resolved effective prefix computed before migration"
        );
    }
}

#[test]
fn test_migrate_v14_to_v15_collision_resolution_matches_sqlite_v9_to_v10_sequence() {
    let mut env = json!({
        "version": 14,
        "metadata": { "instance_id": "00000000-0000-0000-0000-000000000001", "saved_at": "2024-01-01T00:00:00Z" },
        "data": {
            "boards": [
                { "id": "11111111-1111-1111-1111-111111111111", "name": "A", "card_prefix": null },
                { "id": "22222222-2222-2222-2222-222222222222", "name": "B", "card_prefix": null },
                { "id": "33333333-3333-3333-3333-333333333333", "name": "C", "card_prefix": "alpha" },
                { "id": "44444444-4444-4444-4444-444444444444", "name": "D", "card_prefix": "override" }
            ],
            "columns": [], "cards": [], "archived_cards": [], "sprints": [],
            "graph": { "spawns": { "edges": [] }, "blocks": { "edges": [] }, "relates": { "edges": [] } }
        }
    });

    kanban_persistence_json::migration_test_support::transform_v14_to_v15(&mut env);

    let mut names: Vec<String> = env["data"]["prefixes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();

    assert_eq!(
        names,
        vec![
            "alpha".to_string(),
            "override".to_string(),
            "task".to_string(),
            "task2".to_string(),
        ],
        "the JSON migration's collision resolution (task, task2, ...) must match the sequence \
         recorded for the SQLite V9->V10 migration on the same scenario (task, task2, alpha, override)"
    );
}
