//! `cards.prefix` is backed by a foreign key to `prefixes(name)` on a
//! freshly created database (via `schema.sql`), not only after an upgrade.

use kanban_domain::data_store::DataStore;
use kanban_domain::{Board, Card, DomainError, KanbanError, Snapshot};
use sqlx::{Pool, Sqlite};
use tempfile::TempDir;

use super::super::SqliteStore;
use super::make_rt;

const TS: &str = "2024-01-01T00:00:00Z";

async fn seed_board_and_column(pool: &Pool<Sqlite>) -> (uuid::Uuid, uuid::Uuid) {
    let board_id = uuid::Uuid::new_v4();
    let column_id = uuid::Uuid::new_v4();
    sqlx::raw_sql(&format!(
        "INSERT INTO boards (id, name, created_at, updated_at)
             VALUES ('{board_id}','Board','{TS}','{TS}');
         INSERT INTO columns (id, board_id, name, position, created_at, updated_at)
             VALUES ('{column_id}','{board_id}','Todo',0,'{TS}','{TS}');"
    ))
    .execute(pool)
    .await
    .unwrap();
    (board_id, column_id)
}

async fn fk_present(pool: &Pool<Sqlite>) -> bool {
    sqlx::query_scalar::<_, bool>(
        "SELECT COUNT(*) > 0 FROM pragma_foreign_key_list('cards') WHERE \"table\" = 'prefixes'",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[test]
fn test_inserting_a_card_with_an_unbacked_namespace_is_rejected_on_a_fresh_database() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();

    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool().clone();
        let (board_id, column_id) = seed_board_and_column(&pool).await;

        let card_id = uuid::Uuid::new_v4();
        let result = sqlx::raw_sql(&format!(
            "INSERT INTO cards (id, column_id, board_id, title, position, priority, status,
                                card_number, prefix, created_at, updated_at)
                 VALUES ('{card_id}','{column_id}','{board_id}','Card',0,'medium','todo',
                         1,'ZZZ','{TS}','{TS}');"
        ))
        .execute(&pool)
        .await;
        assert!(
            result.is_err(),
            "insert of a card naming an unbacked prefix must fail"
        );
    });

    // Reopen on a fresh pool: enforcement must not be an artefact of the
    // migrating connection.
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool().clone();
        let (_board_id, column_id) = seed_board_and_column(&pool).await;
        let board_id: String = sqlx::query_scalar("SELECT board_id FROM columns WHERE id = ?")
            .bind(column_id.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();

        let card_id = uuid::Uuid::new_v4();
        let result = sqlx::raw_sql(&format!(
            "INSERT INTO cards (id, column_id, board_id, title, position, priority, status,
                                card_number, prefix, created_at, updated_at)
                 VALUES ('{card_id}','{column_id}','{board_id}','Card',0,'medium','todo',
                         1,'ZZZ','{TS}','{TS}');"
        ))
        .execute(&pool)
        .await;
        assert!(
            result.is_err(),
            "a second, non-migrating connection must also enforce the constraint"
        );
    });
}

#[test]
fn test_a_card_stored_in_configured_casing_satisfies_the_foreign_key() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();

    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool().clone();
        let (board_id, column_id) = seed_board_and_column(&pool).await;

        sqlx::raw_sql(
            "INSERT INTO prefixes (name, card_counter, sprint_counter) VALUES ('kan', 0, 0);",
        )
        .execute(&pool)
        .await
        .unwrap();

        let card_id = uuid::Uuid::new_v4();
        sqlx::raw_sql(&format!(
            "INSERT INTO cards (id, column_id, board_id, title, position, priority, status,
                                card_number, prefix, created_at, updated_at)
                 VALUES ('{card_id}','{column_id}','{board_id}','Card',0,'medium','todo',
                         1,'KAN','{TS}','{TS}');"
        ))
        .execute(&pool)
        .await
        .unwrap();

        assert!(
            fk_present(&pool).await,
            "the foreign key must exist on a fresh database"
        );
    });
}

#[test]
fn test_a_card_with_no_prefix_is_still_writable_under_the_foreign_key() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();

    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("Board", None::<String>);
        store.upsert_board(board.clone()).unwrap();
        let column = kanban_domain::Column::new(board.id, "Todo", 0);
        store.upsert_column(column.clone()).unwrap();

        let card = Card::new(board.id, column.id, "Card", 0);
        assert!(card.prefix.is_empty());
        store.upsert_card(card.clone()).unwrap();

        let read_back = store.get_card(card.id).unwrap().unwrap();
        assert!(read_back.prefix.is_empty());
        assert!(fk_present(store.pool()).await);
    });
}

#[test]
fn test_upserting_a_card_with_an_unbacked_namespace_reports_a_domain_error() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();

    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("Board", None::<String>);
        store.upsert_board(board.clone()).unwrap();
        let column = kanban_domain::Column::new(board.id, "Todo", 0);
        store.upsert_column(column.clone()).unwrap();

        let mut card = Card::new(board.id, column.id, "Card", 0);
        card.prefix = "ZZZ".to_string();
        card.card_number = 4;

        let result = store.upsert_card(card);
        match result {
            Err(KanbanError::Domain(DomainError::PrefixNotBacked {
                card_number,
                prefix,
            })) => {
                assert_eq!(card_number, 4);
                assert_eq!(prefix, "ZZZ");
            }
            other => panic!("expected PrefixNotBacked, got {other:?}"),
        }

        // An unrelated foreign-key violation must still report as itself.
        let mut dangling = Card::new(board.id, column.id, "Card2", 1);
        dangling.card_number = 5;
        dangling.sprint_id = Some(uuid::Uuid::new_v4());
        let result = store.upsert_card(dangling);
        match result {
            Err(KanbanError::Database(_)) => {}
            other => {
                panic!("expected a raw Database error for a dangling sprint_id, got {other:?}")
            }
        }
    });
}

#[test]
fn test_apply_snapshot_reports_an_unbacked_namespace_as_a_domain_error() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();

    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();

        let board = Board::new("Board", None::<String>);
        let column = kanban_domain::Column::new(board.id, "Todo", 0);
        let mut card = Card::new(board.id, column.id, "Card", 0);
        card.prefix = "KAN".to_string();
        card.card_number = 7;

        let mut snapshot = Snapshot::new();
        snapshot.boards = vec![board];
        snapshot.columns = vec![column];
        snapshot.cards = vec![card];
        assert!(snapshot.prefixes.is_empty());

        let result = store.apply_snapshot(snapshot);
        match result {
            Err(KanbanError::Domain(DomainError::PrefixNotBacked {
                card_number,
                prefix,
            })) => {
                assert_eq!(card_number, 7);
                assert_eq!(prefix, "KAN");
            }
            other => panic!("expected PrefixNotBacked, got {other:?}"),
        }
    });
}
