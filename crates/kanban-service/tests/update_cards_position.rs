//! `update_cards`'s plain column-move case (column_id set, status/position
//! left None) must recompute position like `move_card`/`move_cards` do. Left
//! unfixed, the card keeps its old position, silently colliding with whatever
//! already sits at that position in the target column.
//!
//! Run against both `JsonDataStore` and `SqliteBackend` (see `move_cards.rs`) since
//! the fix reads `count_cards_in_column_filtered`, a per-backend `DataStore` method.

use kanban_domain::{Board, Card, CardUpdate, Column};
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

macro_rules! update_cards_position_tests {
    ($mod_name:ident, $open_ctx:expr) => {
        mod $mod_name {
            use super::*;

            #[tokio::test(flavor = "multi_thread")]
            async fn test_update_cards_column_move_appends_after_existing_card_in_target() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let mut board = Board::new("B", Some("TST"));
                let col_from = Column::new(board.id, "From", 0);
                let col_to = Column::new(board.id, "To", 1);
                let col_from_id = col_from.id;
                let col_to_id = col_to.id;

                let existing = Card::new(&mut board, col_to_id, "Existing", 0);
                let moving = Card::new(&mut board, col_from_id, "Moving", 0);
                let moving_id = moving.id;
                backend.upsert_board(board).unwrap();
                backend.upsert_column(col_from).unwrap();
                backend.upsert_column(col_to).unwrap();
                backend.upsert_card(existing).unwrap();
                backend.upsert_card(moving).unwrap();

                ctx.update_cards(vec![(
                    moving_id,
                    CardUpdate {
                        column_id: Some(col_to_id),
                        ..Default::default()
                    },
                )])
                .unwrap();

                let moved = backend.get_card(moving_id).unwrap().unwrap();
                assert_eq!(moved.column_id, col_to_id);
                assert_eq!(
                    moved.position, 1,
                    "column-only batch move must append after the existing card, not collide at position 0"
                );
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_update_cards_column_move_multiple_cards_into_same_column_get_sequential_positions(
            ) {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let mut board = Board::new("B", Some("TST"));
                let col_from = Column::new(board.id, "From", 0);
                let col_to = Column::new(board.id, "To", 1);
                let col_from_id = col_from.id;
                let col_to_id = col_to.id;

                // Both start at the same source position so that, pre-fix, retaining
                // the old (unrecomputed) position on both would land them on the same
                // position in the target column too -- the collision this test pins.
                let move1 = Card::new(&mut board, col_from_id, "M1", 0);
                let move2 = Card::new(&mut board, col_from_id, "M2", 0);
                let move1_id = move1.id;
                let move2_id = move2.id;
                backend.upsert_board(board).unwrap();
                backend.upsert_column(col_from).unwrap();
                backend.upsert_column(col_to).unwrap();
                backend.upsert_card(move1).unwrap();
                backend.upsert_card(move2).unwrap();

                ctx.update_cards(vec![
                    (
                        move1_id,
                        CardUpdate {
                            column_id: Some(col_to_id),
                            ..Default::default()
                        },
                    ),
                    (
                        move2_id,
                        CardUpdate {
                            column_id: Some(col_to_id),
                            ..Default::default()
                        },
                    ),
                ])
                .unwrap();

                let mut positions: Vec<i32> = [move1_id, move2_id]
                    .iter()
                    .map(|id| backend.get_card(*id).unwrap().unwrap().position)
                    .collect();
                positions.sort();
                assert_eq!(
                    positions,
                    vec![0, 1],
                    "chained moves into the same column must use distinct positions, not collide"
                );
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_update_cards_reaffirming_current_column_does_not_move_position() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let mut board = Board::new("B", Some("TST"));
                let col = Column::new(board.id, "Col", 0);
                let col_id = col.id;

                let other = Card::new(&mut board, col_id, "Other", 0);
                let card = Card::new(&mut board, col_id, "Card", 1);
                let card_id = card.id;
                backend.upsert_board(board).unwrap();
                backend.upsert_column(col).unwrap();
                backend.upsert_card(other).unwrap();
                backend.upsert_card(card).unwrap();

                // column_id resubmits the card's current column (e.g. a PUT-replace
                // that always sets every field) -- not a real move, so position
                // must be left untouched, not recomputed as an append.
                ctx.update_cards(vec![(
                    card_id,
                    CardUpdate {
                        column_id: Some(col_id),
                        ..Default::default()
                    },
                )])
                .unwrap();

                let unchanged = backend.get_card(card_id).unwrap().unwrap();
                assert_eq!(
                    unchanged.position, 1,
                    "resubmitting the same column must not recompute position"
                );
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_update_cards_column_move_respects_explicit_position() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let mut board = Board::new("B", Some("TST"));
                let col_from = Column::new(board.id, "From", 0);
                let col_to = Column::new(board.id, "To", 1);
                let col_from_id = col_from.id;
                let col_to_id = col_to.id;

                let existing = Card::new(&mut board, col_to_id, "Existing", 0);
                let moving = Card::new(&mut board, col_from_id, "Moving", 0);
                let moving_id = moving.id;
                backend.upsert_board(board).unwrap();
                backend.upsert_column(col_from).unwrap();
                backend.upsert_column(col_to).unwrap();
                backend.upsert_card(existing).unwrap();
                backend.upsert_card(moving).unwrap();

                // Caller pins both column_id and position explicitly -- the
                // recompute must not override a value the caller already chose.
                ctx.update_cards(vec![(
                    moving_id,
                    CardUpdate {
                        column_id: Some(col_to_id),
                        position: Some(0),
                        ..Default::default()
                    },
                )])
                .unwrap();

                let moved = backend.get_card(moving_id).unwrap().unwrap();
                assert_eq!(moved.column_id, col_to_id);
                assert_eq!(
                    moved.position, 0,
                    "an explicit position on the same entry must not be overridden by the recompute"
                );
            }
        }
    };
}

update_cards_position_tests!(json_backend, open_json_ctx());
update_cards_position_tests!(sqlite_backend, open_sqlite_ctx());
