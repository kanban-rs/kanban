//! End-to-end migration + pre-migration-backup coverage across realistic
//! kanban DB states: base (empty), card data (live + archived), and board
//! data (multiple boards + subtrees + sprints). Each opens a seeded schema-2
//! DB through `SqliteStore::open` (which writes the durable backup, then runs
//! the destructive 2->3 rebuild) and asserts BOTH that the live data survives
//! (read back via the `DataStore` API) AND that the backup is a faithful
//! pre-migration schema-2 snapshot.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::TempDir;
use uuid::Uuid;

use super::super::SqliteStore;
use super::make_rt;
use super::migration_v2_to_v3::seed_v2_db;
use kanban_domain::DataStore;

/// Raw schema-2 pool with FKs off (matches `seed_v2_db`).
async fn v2_pool(path: &std::path::Path) -> sqlx::Pool<sqlx::Sqlite> {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .foreign_keys(false),
        )
        .await
        .unwrap()
}

/// Open the backup file as a read-only pool and return its schema_version.
async fn backup_schema_version(path: &std::path::Path) -> u32 {
    let backup = SqliteStore::backup_path_for(path, 2);
    assert!(backup.exists(), "expected a pre-migration .v2.backup file");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(&backup))
        .await
        .unwrap();
    let v: u32 = sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    pool.close().await;
    v
}

async fn store_schema_version(store: &SqliteStore) -> u32 {
    sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
        .fetch_one(store.pool())
        .await
        .unwrap()
}

// ===== BASE CASE: empty schema-2 DB =====

#[test]
fn test_migration_base_case_empty_db_backs_up_and_migrates_clean() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("empty.db");
    let rt = make_rt();
    rt.block_on(async {
        // Seed the v2 schema, then wipe all data rows -> empty v2 DB.
        seed_v2_db(&path, Uuid::nil()).await;
        let p = v2_pool(&path).await;
        for t in [
            "archived_cards",
            "sprint_logs",
            "cards",
            "columns",
            "boards",
        ] {
            sqlx::query(&format!("DELETE FROM {t}"))
                .execute(&p)
                .await
                .unwrap();
        }
        p.close().await;

        let store = SqliteStore::open(&path).await.unwrap();

        assert_eq!(
            store_schema_version(&store).await,
            6,
            "migrated to current v6"
        );
        assert_eq!(
            backup_schema_version(&path).await,
            2,
            "backup is pre-migration v2"
        );
        // Everything reads back empty via the DataStore API.
        assert!(store.list_boards().unwrap().is_empty());
        assert!(store.list_all_cards().unwrap().is_empty());
        assert!(store.list_archived_cards().unwrap().is_empty());
    });
}

// ===== CARDS CASE: live + archived cards survive the destructive rebuild =====

#[test]
fn test_migration_cards_case_preserves_live_and_archived_cards() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("cards.db");
    let rt = make_rt();
    rt.block_on(async {
        // seed_v2_db creates 1 board + 1 column + 1 card that is ARCHIVED.
        let (board_id, column_id, card_id) = seed_v2_db(&path, Uuid::nil()).await;
        // Point the archived row at the real column so board_id backfills, and
        // add a second LIVE card so we cover both live + archived through the
        // `cards` table rebuild.
        let live_card = Uuid::new_v4();
        let p = v2_pool(&path).await;
        sqlx::query("UPDATE archived_cards SET original_column_id = ? WHERE card_id = ?")
            .bind(column_id.to_string())
            .bind(card_id.to_string())
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO cards (id, column_id, title, position, card_number, created_at, updated_at)
             VALUES (?, ?, 'Live', 1, 2, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
        )
        .bind(live_card.to_string())
        .bind(column_id.to_string())
        .execute(&p)
        .await
        .unwrap();
        p.close().await;

        let store = SqliteStore::open(&path).await.unwrap();

        assert_eq!(store_schema_version(&store).await, 6);
        assert_eq!(backup_schema_version(&path).await, 2);

        // Board survived.
        assert!(
            store.get_board(board_id).unwrap().is_some(),
            "board survived"
        );
        // Live card survived and reads back live (not archived).
        let live = store.list_all_cards().unwrap();
        assert!(live.iter().any(|c| c.id == live_card), "live card survived");
        assert!(
            !live.iter().any(|c| c.id == card_id),
            "archived card must NOT appear in the live list"
        );
        // Archived card survived and now carries the backfilled board_id.
        let archived = store.list_archived_cards().unwrap();
        assert_eq!(archived.len(), 1, "archived card survived the rebuild");
        assert_eq!(archived[0].entity_id, card_id);
        assert_eq!(
            archived[0].context.board_id, board_id,
            "board_id backfilled from original_column_id"
        );
        // Board-scoped archived query resolves through the new board_id column.
        assert_eq!(
            store.list_archived_cards_by_board(board_id).unwrap().len(),
            1
        );
    });
}

// ===== BOARDS CASE: multiple boards + subtrees + sprints survive =====

#[test]
fn test_migration_boards_case_preserves_multiple_board_subtrees() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("boards.db");
    let rt = make_rt();
    rt.block_on(async {
        // Base fixture: board A + column + archived card.
        let (board_a, _col_a, _archived) = seed_v2_db(&path, Uuid::nil()).await;

        // Add a SECOND board B with two columns and three live cards + a sprint.
        let board_b = Uuid::new_v4();
        let col_b1 = Uuid::new_v4();
        let col_b2 = Uuid::new_v4();
        let sprint_b = Uuid::new_v4();
        let p = v2_pool(&path).await;
        sqlx::query(
            "INSERT INTO boards (id, name, created_at, updated_at)
             VALUES (?, 'BoardB', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
        )
        .bind(board_b.to_string())
        .execute(&p)
        .await
        .unwrap();
        for (cid, name, pos) in [(col_b1, "Doing", 0), (col_b2, "Done", 1)] {
            sqlx::query(
                "INSERT INTO columns (id, board_id, name, position, created_at, updated_at)
                 VALUES (?, ?, ?, ?, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            )
            .bind(cid.to_string())
            .bind(board_b.to_string())
            .bind(name)
            .bind(pos)
            .execute(&p)
            .await
            .unwrap();
        }
        for (i, col) in [col_b1, col_b1, col_b2].iter().enumerate() {
            sqlx::query(
                "INSERT INTO cards (id, column_id, title, position, card_number, sprint_id, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(col.to_string())
            .bind(format!("BCard{i}"))
            .bind(i as i32)
            .bind(10 + i as i32)
            .bind(if i == 0 { Some(sprint_b.to_string()) } else { None })
            .execute(&p)
            .await
            .unwrap();
        }
        p.close().await;

        let store = SqliteStore::open(&path).await.unwrap();

        assert_eq!(store_schema_version(&store).await, 6);
        assert_eq!(backup_schema_version(&path).await, 2);

        // Both boards survived with their subtrees.
        assert_eq!(store.list_boards().unwrap().len(), 2, "both boards survived");
        assert!(store.get_board(board_a).unwrap().is_some());
        assert!(store.get_board(board_b).unwrap().is_some());
        assert_eq!(store.list_columns_by_board(board_b).unwrap().len(), 2);
        // 3 live cards on B (A's only card is archived).
        let live = store.list_all_cards().unwrap();
        assert_eq!(live.len(), 3, "board B's 3 live cards survived");
        // The sprint assignment on the first B card survived.
        assert_eq!(store.list_cards_by_sprint(sprint_b).unwrap().len(), 1);
        // A's archived card is still archived.
        assert_eq!(store.list_archived_cards().unwrap().len(), 1);
    });
}

// ===== IDEMPOTENCY: re-open the migrated DB writes no second backup =====

#[test]
fn test_migration_reopen_is_idempotent_and_writes_no_new_backup() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("reopen.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v2_db(&path, Uuid::nil()).await;

        // First open: migrates + writes the v2 backup.
        {
            let store = SqliteStore::open(&path).await.unwrap();
            assert_eq!(store_schema_version(&store).await, 6);
        }
        let backup = SqliteStore::backup_path_for(&path, 2);
        assert!(backup.exists());
        let backup_mtime = std::fs::metadata(&backup).unwrap().modified().unwrap();

        // Second open on the now-v3 DB: no migration, backup untouched.
        {
            let store = SqliteStore::open(&path).await.unwrap();
            assert_eq!(store_schema_version(&store).await, 6);
            assert_eq!(store.list_archived_cards().unwrap().len(), 1, "data intact");
        }
        assert_eq!(
            std::fs::metadata(&backup).unwrap().modified().unwrap(),
            backup_mtime,
            "re-open must not rewrite the backup"
        );
    });
}
