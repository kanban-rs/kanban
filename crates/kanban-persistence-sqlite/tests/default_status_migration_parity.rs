use kanban_domain::data_store::DataStore;
use kanban_persistence::PersistenceStore;
use kanban_persistence_json::JsonFileStore;
use kanban_persistence_sqlite::SqliteStore;
use serde_json::{json, Value};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::TempDir;
use uuid::Uuid;

/// Write a V13 JSON envelope (pre-derivation: every column carries a `null`
/// `default_status`) with one board whose `completion_column_ids` names
/// `done_column_id`, and two columns.
async fn write_v13_json(
    path: &std::path::Path,
    board_id: Uuid,
    done_column_id: Uuid,
    other_column_id: Uuid,
) {
    let envelope = json!({
        "version": 13,
        "metadata": {
            "instance_id": "00000000-0000-0000-0000-000000000001",
            "saved_at": "2024-01-01T00:00:00Z"
        },
        "data": {
            "boards": [{
                "id": board_id,
                "name": "B",
                "completion_column_ids": [done_column_id],
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            }],
            "columns": [
                {
                    "id": done_column_id, "board_id": board_id, "name": "Complete",
                    "position": 1, "default_status": Value::Null,
                    "created_at": "2024-01-01T00:00:00Z", "updated_at": "2024-01-01T00:00:00Z"
                },
                {
                    "id": other_column_id, "board_id": board_id, "name": "Doing",
                    "position": 0, "default_status": Value::Null,
                    "created_at": "2024-01-01T00:00:00Z", "updated_at": "2024-01-01T00:00:00Z"
                }
            ],
            "cards": [],
            "archived_cards": [],
            "sprints": [],
            "graph": {
                "spawns": { "edges": [] },
                "blocks": { "edges": [] },
                "relates": { "edges": [] }
            }
        }
    });
    tokio::fs::write(path, serde_json::to_string_pretty(&envelope).unwrap())
        .await
        .unwrap();
}

/// Read back a column's `default_status` from a JSON store's loaded data.
async fn json_default_status(path: &std::path::Path, column_id: Uuid) -> Option<String> {
    let store = JsonFileStore::new(path);
    let (snapshot, _) = store.load().await.unwrap();
    let data: Value = serde_json::from_slice(&snapshot.data).unwrap();
    data["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == column_id.to_string())
        .and_then(|c| c["default_status"].as_str())
        .map(str::to_string)
}

/// Seed a schema_version-7 shaped SQLite DB directly: `columns.default_status`
/// already exists (nullable), `board_completion_columns` already exists.
async fn write_v7_sqlite(
    path: &std::path::Path,
    board_id: Uuid,
    done_column_id: Uuid,
    other_column_id: Uuid,
) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .foreign_keys(false),
        )
        .await
        .unwrap();

    sqlx::raw_sql(
        "CREATE TABLE metadata (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            instance_id TEXT NOT NULL,
            saved_at TEXT NOT NULL,
            schema_version INTEGER NOT NULL,
            writer_version TEXT,
            writer_commit TEXT
        );
        CREATE TABLE boards (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT,
            sprint_prefix TEXT, card_prefix TEXT,
            task_sort_field TEXT NOT NULL DEFAULT 'Default',
            task_sort_order TEXT NOT NULL DEFAULT 'Ascending',
            sprint_duration_days INTEGER,
            sprint_name_used_count INTEGER NOT NULL DEFAULT 0,
            next_sprint_number INTEGER NOT NULL DEFAULT 1,
            active_sprint_id TEXT,
            task_list_view TEXT NOT NULL DEFAULT 'Flat',
            card_counter INTEGER NOT NULL DEFAULT 1,
            position INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE TABLE columns (
            id TEXT PRIMARY KEY, board_id TEXT NOT NULL, name TEXT NOT NULL,
            position INTEGER NOT NULL, wip_limit INTEGER, default_status TEXT,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
            FOREIGN KEY (board_id) REFERENCES boards(id) ON DELETE CASCADE
        );
        CREATE TABLE board_completion_columns (
            board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
            column_id TEXT NOT NULL REFERENCES columns(id) ON DELETE CASCADE,
            position INTEGER NOT NULL,
            PRIMARY KEY (board_id, column_id)
        );",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO metadata (id, instance_id, saved_at, schema_version)
         VALUES (1, ?, '2024-01-01T00:00:00Z', 7)",
    )
    .bind(Uuid::new_v4().to_string())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO boards (id, name, created_at, updated_at)
         VALUES (?, 'B', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
    )
    .bind(board_id.to_string())
    .execute(&pool)
    .await
    .unwrap();
    for (cid, name, pos) in [
        (done_column_id, "Complete", 1),
        (other_column_id, "Doing", 0),
    ] {
        sqlx::query(
            "INSERT INTO columns (id, board_id, name, position, created_at, updated_at)
             VALUES (?, ?, ?, ?, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
        )
        .bind(cid.to_string())
        .bind(board_id.to_string())
        .bind(name)
        .bind(pos)
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO board_completion_columns (board_id, column_id, position)
         VALUES (?, ?, 0)",
    )
    .bind(board_id.to_string())
    .bind(done_column_id.to_string())
    .execute(&pool)
    .await
    .unwrap();

    pool.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_json_and_sqlite_derive_identical_default_status_for_the_same_graph() {
    let dir = TempDir::new().unwrap();
    let board_id = Uuid::new_v4();
    let done_column_id = Uuid::new_v4();
    let other_column_id = Uuid::new_v4();

    let json_path = dir.path().join("board.json");
    write_v13_json(&json_path, board_id, done_column_id, other_column_id).await;

    let sqlite_path = dir.path().join("board.sqlite3");
    write_v7_sqlite(&sqlite_path, board_id, done_column_id, other_column_id).await;

    let json_done_status = json_default_status(&json_path, done_column_id).await;
    let json_other_status = json_default_status(&json_path, other_column_id).await;

    let store = SqliteStore::open(&sqlite_path).await.unwrap();
    let sqlite_columns = store.list_columns_by_board(board_id).unwrap();
    let sqlite_done_status = sqlite_columns
        .iter()
        .find(|c| c.id == done_column_id)
        .and_then(|c| c.default_status)
        .map(|s| {
            serde_json::to_value(s)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string()
        });
    let sqlite_other_status = sqlite_columns
        .iter()
        .find(|c| c.id == other_column_id)
        .and_then(|c| c.default_status)
        .map(|s| {
            serde_json::to_value(s)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string()
        });

    assert_eq!(
        json_done_status, sqlite_done_status,
        "the completion column must derive the same default_status on both backends"
    );
    assert_eq!(
        json_other_status, sqlite_other_status,
        "the non-completion column must derive the same default_status on both backends"
    );
    assert_eq!(json_done_status, Some("Done".to_string()));
    assert_eq!(json_other_status, Some("Todo".to_string()));
}
