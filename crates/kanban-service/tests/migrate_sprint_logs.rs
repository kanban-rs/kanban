//! Integration tests for `KanbanContext::migrate_sprint_logs` (KAN-430).
//!
//! Run against both `JsonDataStore` and `SqliteBackend` via a macro to catch any
//! backend-specific divergence. The pure migration logic itself is unit-tested
//! in `kanban_domain::card_lifecycle::tests`.

use kanban_domain::{Board, Card, Column, Sprint};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_persistence_sqlite::SqliteBackend;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;
use tempfile::tempdir;

async fn open_json_ctx() -> (KanbanContext, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.json");
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(&path))));
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    (ctx, dir)
}

async fn open_sqlite_ctx() -> (KanbanContext, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.sqlite");
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(SqliteBackend::open(path.to_str().unwrap()).await.unwrap());
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    (ctx, dir)
}

macro_rules! migrate_sprint_logs_tests {
    ($mod_name:ident, $open_ctx:expr) => {
        mod $mod_name {
            use super::*;

            #[tokio::test(flavor = "multi_thread")]
            async fn test_migrate_sprint_logs_backfills_card_with_sprint_id_and_empty_logs() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let mut board = Board::new("B", Some("TST"));
                let col = Column::new(board.id, "Col", 0);
                let sprint = Sprint::new(board.id, 1, None, Some("Alpha"));
                let sprint_id = sprint.id;
                let mut card = Card::new(&mut board, col.id, "Card", 0);
                let card_id = card.id;
                card.sprint_id = Some(sprint_id);
                assert!(card.sprint_logs.is_empty());
                backend.upsert_board(board).unwrap();
                backend.upsert_column(col).unwrap();
                backend.upsert_sprint(sprint).unwrap();
                backend.upsert_card(card).unwrap();

                let migrated = ctx.migrate_sprint_logs().unwrap();
                assert_eq!(migrated, 1);

                let card = backend.get_card(card_id).unwrap().unwrap();
                assert_eq!(
                    card.sprint_logs.len(),
                    1,
                    "sprint log should be backfilled for card with sprint_id but empty logs"
                );
                assert_eq!(card.sprint_logs[0].sprint_number, 1);
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_migrate_sprint_logs_no_op_when_nothing_to_migrate() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let mut board = Board::new("B", Some("TST"));
                let col = Column::new(board.id, "Col", 0);
                let card = Card::new(&mut board, col.id, "Card", 0);
                backend.upsert_board(board).unwrap();
                backend.upsert_column(col).unwrap();
                backend.upsert_card(card).unwrap();

                let before = backend.list_all_cards().unwrap();
                let migrated = ctx.migrate_sprint_logs().unwrap();
                assert_eq!(
                    migrated, 0,
                    "migrate_sprint_logs should report zero when no card needs backfilling"
                );
                assert_eq!(
                    backend.list_all_cards().unwrap(),
                    before,
                    "no-op migration must not mutate any card"
                );
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_migrate_sprint_logs_only_backfills_eligible_cards_in_mixed_batch() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let mut board = Board::new("B", Some("TST"));
                let col = Column::new(board.id, "Col", 0);
                let sprint = Sprint::new(board.id, 1, None, Some("Alpha"));
                let sprint_id = sprint.id;

                let mut card_needs_backfill = Card::new(&mut board, col.id, "Needs Backfill", 0);
                card_needs_backfill.sprint_id = Some(sprint_id);
                let needs_backfill_id = card_needs_backfill.id;

                let mut card_already_logged = Card::new(&mut board, col.id, "Already Logged", 1);
                card_already_logged.sprint_id = Some(sprint_id);
                card_already_logged
                    .sprint_logs
                    .push(kanban_domain::SprintLog::new(
                        sprint_id,
                        1,
                        None::<String>,
                        "Active",
                    ));
                let already_logged_id = card_already_logged.id;
                let already_logged_before = card_already_logged.sprint_logs.clone();

                let card_no_sprint = Card::new(&mut board, col.id, "No Sprint", 2);
                let no_sprint_id = card_no_sprint.id;

                backend.upsert_board(board).unwrap();
                backend.upsert_column(col).unwrap();
                backend.upsert_sprint(sprint).unwrap();
                backend.upsert_card(card_needs_backfill).unwrap();
                backend.upsert_card(card_already_logged).unwrap();
                backend.upsert_card(card_no_sprint).unwrap();

                let migrated = ctx.migrate_sprint_logs().unwrap();
                assert_eq!(migrated, 1, "only the eligible card should be migrated");

                let backfilled = backend.get_card(needs_backfill_id).unwrap().unwrap();
                assert_eq!(backfilled.sprint_logs.len(), 1);
                assert_eq!(backfilled.sprint_logs[0].sprint_number, 1);

                let already_logged = backend.get_card(already_logged_id).unwrap().unwrap();
                assert_eq!(
                    already_logged.sprint_logs, already_logged_before,
                    "card with existing logs must be untouched"
                );

                let no_sprint = backend.get_card(no_sprint_id).unwrap().unwrap();
                assert!(
                    no_sprint.sprint_logs.is_empty(),
                    "card without sprint_id must remain unlogged"
                );
            }
        }
    };
}

migrate_sprint_logs_tests!(json_backend, open_json_ctx());
migrate_sprint_logs_tests!(sqlite_backend, open_sqlite_ctx());
