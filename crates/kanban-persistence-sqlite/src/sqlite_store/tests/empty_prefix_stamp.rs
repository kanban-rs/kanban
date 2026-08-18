//! Coverage for `SqliteStore::stamp_empty_card_prefixes`: every card that
//! carries no prefix is given the one it is addressed by, on open, before
//! `repair_unbacked_card_namespaces` runs.

use sqlx::{Pool, Sqlite};
use tempfile::TempDir;

use super::super::SqliteStore;
use super::make_rt;

const TS: &str = "2024-01-01T00:00:00Z";

#[allow(clippy::too_many_arguments)]
async fn seed(
    pool: &Pool<Sqlite>,
    board_id: &str,
    board_prefix: &str,
    column_id: &str,
    card_id: &str,
    card_prefix: &str,
    number: i64,
) {
    sqlx::raw_sql(&format!(
        "INSERT INTO boards (id, name, card_prefix, created_at, updated_at)
             VALUES ('{board_id}','Board','{board_prefix}','{TS}','{TS}')
             ON CONFLICT(id) DO NOTHING;
         INSERT INTO columns (id, board_id, name, position, created_at, updated_at)
             VALUES ('{column_id}','{board_id}','Todo',0,'{TS}','{TS}')
             ON CONFLICT(id) DO NOTHING;
         INSERT INTO cards (id, column_id, board_id, title, position, priority, status,
                            card_number, prefix, created_at, updated_at)
             VALUES ('{card_id}','{column_id}','{board_id}','Card',0,'medium','todo',
                     {number},'{card_prefix}','{TS}','{TS}');"
    ))
    .execute(pool)
    .await
    .unwrap();
}

async fn card_prefix(pool: &Pool<Sqlite>, card_id: &str) -> String {
    sqlx::query_scalar("SELECT prefix FROM cards WHERE id = ?")
        .bind(card_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn prefix_counter(pool: &Pool<Sqlite>, name: &str) -> Option<i64> {
    sqlx::query_scalar("SELECT card_counter FROM prefixes WHERE name = ?")
        .bind(name)
        .fetch_optional(pool)
        .await
        .unwrap()
}

#[test]
fn test_an_empty_prefix_card_is_stamped_during_the_upgrade() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();
    rt.block_on(async {
        let board = "00000000-0000-0000-0000-0000000000b1";
        let column = "00000000-0000-0000-0000-0000000000c1";
        let card = "00000000-0000-0000-0000-0000000000a1";

        {
            let store = SqliteStore::open(&path).await.unwrap();
            let pool = store.pool();
            seed(pool, board, "KAN", column, card, "", 7).await;
        }

        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();

        assert_eq!(card_prefix(pool, card).await, "KAN");
        assert_eq!(prefix_counter(pool, "kan").await, Some(7));
    });
}

#[test]
fn test_a_card_whose_column_names_no_board_takes_the_builtin_prefix() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();
    rt.block_on(async {
        let board = "00000000-0000-0000-0000-0000000000b1";
        let dangling_column = "00000000-0000-0000-0000-0000000000cd";
        let card = "00000000-0000-0000-0000-0000000000a1";

        {
            let store = SqliteStore::open(&path).await.unwrap();
            let pool = store.pool();
            sqlx::raw_sql(&format!(
                "INSERT INTO boards (id, name, card_prefix, created_at, updated_at)
                     VALUES ('{board}','Board','KAN','{TS}','{TS}');
                 INSERT INTO cards (id, column_id, board_id, title, position, priority, status,
                                    card_number, prefix, created_at, updated_at)
                     VALUES ('{card}','{dangling_column}','{board}','Card',0,'medium','todo',
                             9,'','{TS}','{TS}');"
            ))
            .execute(pool)
            .await
            .unwrap();
        }

        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();

        assert_eq!(card_prefix(pool, card).await, "task");
        assert_eq!(prefix_counter(pool, "task").await, Some(9));
    });
}

#[test]
fn test_a_card_whose_column_id_is_not_a_uuid_still_loses_its_empty_prefix() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();
    rt.block_on(async {
        let board = "00000000-0000-0000-0000-0000000000b1";
        let sprint = "00000000-0000-0000-0000-0000000000f1";
        let card = "00000000-0000-0000-0000-0000000000a1";

        {
            let store = SqliteStore::open(&path).await.unwrap();
            let pool = store.pool();
            sqlx::raw_sql(&format!(
                "INSERT INTO boards (id, name, card_prefix, created_at, updated_at)
                     VALUES ('{board}','Board','KAN','{TS}','{TS}');
                 INSERT INTO sprints (id, board_id, sprint_number, status, card_prefix, created_at, updated_at)
                     VALUES ('{sprint}','{board}',1,'Planning','AUTH','{TS}','{TS}');
                 INSERT INTO cards (id, column_id, board_id, title, position, priority, status,
                                    card_number, prefix, sprint_id, created_at, updated_at)
                     VALUES ('{card}','not-a-uuid','{board}','Card',0,'medium','todo',
                             2,'','{sprint}','{TS}','{TS}');"
            ))
            .execute(pool)
            .await
            .unwrap();
        }

        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();

        assert_eq!(card_prefix(pool, card).await, "task");
        let empty: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cards WHERE prefix = ''")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(empty, 0);
    });
}

#[test]
fn test_a_stamped_prefix_is_never_rewritten_by_the_sweep() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();
    rt.block_on(async {
        let board = "00000000-0000-0000-0000-0000000000b1";
        let column = "00000000-0000-0000-0000-0000000000c1";
        let card = "00000000-0000-0000-0000-0000000000a1";

        {
            let store = SqliteStore::open(&path).await.unwrap();
            let pool = store.pool();
            seed(pool, board, "OLD", column, card, "OLD", 3).await;
            sqlx::query("UPDATE boards SET card_prefix = 'NEW' WHERE id = ?")
                .bind(board)
                .execute(pool)
                .await
                .unwrap();
        }

        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();

        assert_eq!(card_prefix(pool, card).await, "OLD");
    });
}
