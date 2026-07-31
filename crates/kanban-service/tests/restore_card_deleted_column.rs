//! Tests for restore_card behavior when the original column has been deleted.
//! KAN-907: Surface actionable hint rather than generic not_found error.

use kanban_domain::{Board, Card, Column, KanbanOperations};
use kanban_persistence_json::JsonFileStore;
use kanban_persistence_sqlite::SqliteBackend;
use kanban_service::{json_backend::JsonDataStore, AppConfig, KanbanBackend, KanbanContext};
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

fn seed_board_column_card(backend: Arc<dyn KanbanBackend>) -> (uuid::Uuid, uuid::Uuid, uuid::Uuid) {
    let mut board = Board::new("Test", Some("TST"));
    let col = Column::new(board.id, "TODO", 0);
    let col_id = col.id;
    let card = Card::new(&mut board, col_id, "Test Card", 0);
    let card_id = card.id;
    let board_id = board.id;
    backend.upsert_board(board).unwrap();
    backend.upsert_column(col).unwrap();
    backend.upsert_card(card).unwrap();
    (board_id, col_id, card_id)
}

macro_rules! restore_card_deleted_column_tests {
    ($mod_name:ident, $open_ctx:expr) => {
        mod $mod_name {
            use super::*;

            #[tokio::test(flavor = "multi_thread")]
            async fn test_restore_card_with_deleted_original_column_and_no_target_returns_actionable_hint() {
                let (mut ctx, _dir) = $open_ctx.await;
                let (_board_id, col_id, card_id) = seed_board_column_card(ctx.backend());

                ctx.archive_card(card_id).unwrap();
                ctx.delete_column(col_id).unwrap();

                let err = ctx
                    .restore_card(card_id, None)
                    .expect_err("restore to deleted column must fail");

                let msg = err.to_string();
                assert!(
                    msg.contains("Original column no longer exists"),
                    "error must mention 'Original column no longer exists', got: {msg}"
                );
                assert!(
                    msg.contains("column-id"),
                    "error must mention 'column-id' hint, got: {msg}"
                );
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_restore_card_with_deleted_original_column_but_explicit_target_succeeds() {
                let (mut ctx, _dir) = $open_ctx.await;
                let (board_id, col_id, card_id) = seed_board_column_card(ctx.backend());

                ctx.archive_card(card_id).unwrap();
                ctx.delete_column(col_id).unwrap();

                // Create an alternative column.
                let other_col = ctx
                    .create_column(board_id, "Done".into(), None)
                    .unwrap();

                let restored = ctx
                    .restore_card(card_id, Some(other_col.id))
                    .expect("restore to explicit column must succeed");

                assert_eq!(
                    restored.column_id, other_col.id,
                    "restored card must be in the explicitly-specified column"
                );
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_restore_card_default_branch_with_live_column_still_succeeds() {
                let (mut ctx, _dir) = $open_ctx.await;
                let (_board_id, col_id, card_id) = seed_board_column_card(ctx.backend());

                ctx.archive_card(card_id).unwrap();

                // Column is still alive; default restore must succeed.
                let restored = ctx
                    .restore_card(card_id, None)
                    .expect("restore to live column must succeed");

                assert_eq!(
                    restored.column_id, col_id,
                    "restored card must be in its original column"
                );
            }
        }
    };
}

restore_card_deleted_column_tests!(json, open_json_ctx());
restore_card_deleted_column_tests!(sqlite, open_sqlite_ctx());
