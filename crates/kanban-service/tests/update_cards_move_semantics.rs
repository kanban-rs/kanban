//! `update_cards`' plain column-move branch (column_id set, status left None)
//! mutates `column_id`/`position` via a single `UpdateCard` command instead of
//! chaining a `MoveCard` like `move_card`/`move_cards` do. `UpdateCard::execute`
//! doesn't enforce the destination column's WIP limit and doesn't sync
//! `card.board_id` to the target column's board -- both of which
//! `MoveCard::execute` does. This is reachable through the TUI's real bulk
//! multi-select move (`move_selected_cards` in card_handlers.rs), not just
//! theoretical API misuse.
//!
//! Run against both `JsonDataStore` and `SqliteBackend` (see `move_cards.rs`).

use kanban_domain::{CardUpdate, FieldUpdate};
use kanban_persistence_json::JsonFileStore;
use kanban_service::{
    json_backend::JsonDataStore, sqlite_backend::SqliteBackend, AppConfig, KanbanBackend,
    KanbanContext, KanbanOperations,
};
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

macro_rules! update_cards_move_semantics_tests {
    ($mod_name:ident, $open_ctx:expr) => {
        mod $mod_name {
            use super::*;

            #[tokio::test(flavor = "multi_thread")]
            async fn test_update_cards_column_move_into_wip_limited_column_returns_error_and_leaves_card_unmoved(
            ) {
                // Setup must go through ctx so the cards survive snapshot rollback --
                // direct backend writes are not in the command log and are wiped
                // back to the open()-time baseline on rollback (see move_cards.rs).
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let board = ctx.create_board("B".into(), Some("TST".into())).unwrap();
                let src_col = ctx.create_column(board.id, "Src".into(), None).unwrap();
                let dst_col = ctx.create_column(board.id, "Dst".into(), None).unwrap();
                ctx.update_column(
                    dst_col.id,
                    kanban_domain::ColumnUpdate {
                        wip_limit: FieldUpdate::Set(1),
                        ..Default::default()
                    },
                )
                .unwrap();

                let filler = ctx
                    .create_card(board.id, dst_col.id, "Filler".into(), Default::default())
                    .unwrap();
                let moving = ctx
                    .create_card(board.id, src_col.id, "Moving".into(), Default::default())
                    .unwrap();
                let src_id = src_col.id;

                let result = ctx.update_cards(vec![(
                    moving.id,
                    CardUpdate {
                        column_id: Some(dst_col.id),
                        ..Default::default()
                    },
                )]);

                assert!(
                    result.is_err(),
                    "moving into a WIP-limited column already at its limit must error, like move_cards does"
                );

                let unmoved = backend.get_card(moving.id).unwrap().unwrap();
                assert_eq!(
                    unmoved.column_id, src_id,
                    "a rejected move must leave the card in its original column"
                );
                let filler_still_there = backend.get_card(filler.id).unwrap().unwrap();
                assert_eq!(filler_still_there.column_id, dst_col.id);
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_update_cards_column_move_across_boards_syncs_board_id() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let board_a = ctx.create_board("A".into(), Some("A".into())).unwrap();
                let board_b = ctx.create_board("B".into(), Some("B".into())).unwrap();
                let col_a = ctx.create_column(board_a.id, "ColA".into(), None).unwrap();
                let col_b = ctx.create_column(board_b.id, "ColB".into(), None).unwrap();

                let card = ctx
                    .create_card(board_a.id, col_a.id, "Card".into(), Default::default())
                    .unwrap();

                ctx.update_cards(vec![(
                    card.id,
                    CardUpdate {
                        column_id: Some(col_b.id),
                        ..Default::default()
                    },
                )])
                .unwrap();

                let moved = backend.get_card(card.id).unwrap().unwrap();
                assert_eq!(moved.column_id, col_b.id);
                assert_eq!(
                    moved.board_id, board_b.id,
                    "a cross-board column move must sync board_id to the new column's board, like move_card does"
                );
            }
        }
    };
}

update_cards_move_semantics_tests!(json_backend, open_json_ctx());
update_cards_move_semantics_tests!(sqlite_backend, open_sqlite_ctx());
