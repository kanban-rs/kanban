//! Integration tests for `KanbanContext::move_cards` / `move_cards_detailed` (KAN-428).
//!
//! Run against both `JsonDataStore` and `SqliteBackend` via a macro to catch any
//! backend-specific divergence. Position-computation logic is unit-tested in
//! `kanban_domain::card_lifecycle::tests::compute_move_positions_*`.

use kanban_domain::commands::{CardCommand, Command};
use kanban_domain::{Board, Card, Column};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_persistence_sqlite::SqliteBackend;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext, KanbanOperations};
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

macro_rules! move_cards_tests {
    ($mod_name:ident, $open_ctx:expr) => {
        mod $mod_name {
            use super::*;

            #[tokio::test(flavor = "multi_thread")]
            async fn test_move_cards_appends_after_existing_cards_in_target_column() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let mut board = Board::new("B", Some("TST"));
                let col_from = Column::new(board.id, "From", 0);
                let col_to = Column::new(board.id, "To", 1);
                let col_to_id = col_to.id;

                let existing1 = Card::new(&mut board, col_to_id, "E1", 0);
                let existing2 = Card::new(&mut board, col_to_id, "E2", 1);
                let move1 = Card::new(&mut board, col_from.id, "M1", 0);
                let move2 = Card::new(&mut board, col_from.id, "M2", 1);
                let move1_id = move1.id;
                let move2_id = move2.id;
                backend.upsert_board(board).unwrap();
                backend.upsert_column(col_from).unwrap();
                backend.upsert_column(col_to).unwrap();
                backend.upsert_card(existing1).unwrap();
                backend.upsert_card(existing2).unwrap();
                backend.upsert_card(move1).unwrap();
                backend.upsert_card(move2).unwrap();

                ctx.move_cards(vec![move1_id, move2_id], col_to_id).unwrap();

                let m1 = backend.get_card(move1_id).unwrap().unwrap();
                let m2 = backend.get_card(move2_id).unwrap().unwrap();
                assert_eq!(m1.column_id, col_to_id);
                assert_eq!(m2.column_id, col_to_id);
                assert_eq!(m1.position, 2);
                assert_eq!(m2.position, 3);
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_move_cards_within_same_column_excludes_moving_cards_from_base() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let mut board = Board::new("B", Some("TST"));
                let col = Column::new(board.id, "Col", 0);
                let col_id = col.id;
                let card1 = Card::new(&mut board, col_id, "C1", 0);
                let card2 = Card::new(&mut board, col_id, "C2", 1);
                let card3 = Card::new(&mut board, col_id, "C3", 2);
                let c1_id = card1.id;
                let c3_id = card3.id;
                backend.upsert_board(board).unwrap();
                backend.upsert_column(col).unwrap();
                backend.upsert_card(card1).unwrap();
                backend.upsert_card(card2).unwrap();
                backend.upsert_card(card3).unwrap();

                ctx.move_cards(vec![c1_id, c3_id], col_id).unwrap();

                let c1 = backend.get_card(c1_id).unwrap().unwrap();
                let c3 = backend.get_card(c3_id).unwrap().unwrap();
                assert_eq!(c1.position, 1, "first moved card should be at base(1) + 0");
                assert_eq!(c3.position, 2, "second moved card should be at base(1) + 1");
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_move_cards_exceeding_wip_limit_returns_error_and_rolls_back() {
                // Setup must go through ctx so the cards survive snapshot rollback —
                // direct backend writes are not in the command log and are wiped
                // back to the open()-time baseline on rollback (SQLite uses
                // indexed snapshots seeded at open).
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let board = ctx.create_board("B".into(), Some("TST".into())).unwrap();
                let src_col = ctx.create_column(board.id, "Src".into(), None).unwrap();
                let dst_col = ctx.create_column(board.id, "Dst".into(), None).unwrap();
                ctx.update_column(
                    dst_col.id,
                    kanban_domain::ColumnUpdate {
                        wip_limit: kanban_domain::FieldUpdate::Set(1),
                        ..Default::default()
                    },
                )
                .unwrap();
                let card1 = ctx
                    .create_card(board.id, src_col.id, "C1".into(), Default::default())
                    .unwrap();
                let card2 = ctx
                    .create_card(board.id, src_col.id, "C2".into(), Default::default())
                    .unwrap();
                let src_id = src_col.id;
                let dst_id = dst_col.id;

                let result = ctx.move_cards(vec![card1.id, card2.id], dst_id);
                assert!(
                    result.is_err(),
                    "moving 2 cards into limit=1 column must error"
                );

                // Atomic: nothing moved (snapshot rollback)
                let c1 = backend.get_card(card1.id).unwrap().unwrap();
                let c2 = backend.get_card(card2.id).unwrap().unwrap();
                assert_eq!(c1.column_id, src_id);
                assert_eq!(c2.column_id, src_id);
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_move_cards_detailed_reports_invalid_ids_as_failures() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let mut board = Board::new("B", Some("TST"));
                let col_from = Column::new(board.id, "From", 0);
                let col_to = Column::new(board.id, "To", 1);
                let col_to_id = col_to.id;
                let card = Card::new(&mut board, col_from.id, "C", 0);
                let valid_id = card.id;
                let invalid_id = uuid::Uuid::new_v4();
                backend.upsert_board(board).unwrap();
                backend.upsert_column(col_from).unwrap();
                backend.upsert_column(col_to).unwrap();
                backend.upsert_card(card).unwrap();

                let result = ctx.move_cards_detailed(vec![valid_id, invalid_id], col_to_id);

                assert_eq!(result.succeeded, vec![valid_id]);
                assert_eq!(result.failed.len(), 1);
                assert_eq!(result.failed[0].id, invalid_id);

                let valid = backend.get_card(valid_id).unwrap().unwrap();
                assert_eq!(valid.column_id, col_to_id);
            }

            // KAN-428 followup: pin behaviour change — move_cards now errors and
            // rolls back when any input ID is unknown (previously, invalid IDs
            // were silently skipped by the removed MoveCards batch command).
            // Callers that need partial-success semantics use move_cards_detailed.
            #[tokio::test(flavor = "multi_thread")]
            async fn test_move_cards_with_invalid_id_errors_and_rolls_back() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let board = ctx.create_board("B".into(), Some("TST".into())).unwrap();
                let col_from = ctx.create_column(board.id, "From".into(), None).unwrap();
                let col_to = ctx.create_column(board.id, "To".into(), None).unwrap();
                let card = ctx
                    .create_card(board.id, col_from.id, "C".into(), Default::default())
                    .unwrap();
                let invalid_id = uuid::Uuid::new_v4();
                let from_id = col_from.id;
                let to_id = col_to.id;

                let result = ctx.move_cards(vec![card.id, invalid_id], to_id);
                assert!(
                    result.is_err(),
                    "move_cards with any invalid ID must error after KAN-428"
                );

                let fetched = backend.get_card(card.id).unwrap().unwrap();
                assert_eq!(
                    fetched.column_id, from_id,
                    "valid card must not have moved (snapshot rollback)"
                );
            }

            // KAN-428 followup: the service-level batch WIP pre-check produces a
            // single WipLimitExceeded error before any per-card MoveCard runs,
            // restoring the original batch-level error semantics.
            #[tokio::test(flavor = "multi_thread")]
            async fn test_move_cards_exceeding_wip_limit_returns_single_batch_error() {
                let (mut ctx, _dir) = $open_ctx.await;

                let board = ctx.create_board("B".into(), Some("TST".into())).unwrap();
                let src_col = ctx.create_column(board.id, "Src".into(), None).unwrap();
                let dst_col = ctx.create_column(board.id, "Dst".into(), None).unwrap();
                ctx.update_column(
                    dst_col.id,
                    kanban_domain::ColumnUpdate {
                        wip_limit: kanban_domain::FieldUpdate::Set(1),
                        ..Default::default()
                    },
                )
                .unwrap();
                let c1 = ctx
                    .create_card(board.id, src_col.id, "C1".into(), Default::default())
                    .unwrap();
                let c2 = ctx
                    .create_card(board.id, src_col.id, "C2".into(), Default::default())
                    .unwrap();
                let c3 = ctx
                    .create_card(board.id, src_col.id, "C3".into(), Default::default())
                    .unwrap();

                let err = ctx
                    .move_cards(vec![c1.id, c2.id, c3.id], dst_col.id)
                    .unwrap_err();
                assert!(
                    err.is_wip_limit_exceeded(),
                    "expected WipLimitExceeded, got {err:?}"
                );
            }

            // KAN-428 followup: when invalid ids would *also* push the count
            // past a WIP limit, the pre-existence check should surface
            // not_found first — the WipLimitExceeded that the batch WIP
            // pre-check would otherwise produce is misleading.
            #[tokio::test(flavor = "multi_thread")]
            async fn test_move_cards_invalid_id_into_wip_limited_column_returns_not_found() {
                let (mut ctx, _dir) = $open_ctx.await;

                let board = ctx.create_board("B".into(), Some("TST".into())).unwrap();
                let src_col = ctx.create_column(board.id, "Src".into(), None).unwrap();
                let dst_col = ctx.create_column(board.id, "Dst".into(), None).unwrap();
                // Limit the destination column to 1 so any oversized batch would trip WIP.
                ctx.update_column(
                    dst_col.id,
                    kanban_domain::ColumnUpdate {
                        wip_limit: kanban_domain::FieldUpdate::Set(1),
                        ..Default::default()
                    },
                )
                .unwrap();
                let valid = ctx
                    .create_card(board.id, src_col.id, "C1".into(), Default::default())
                    .unwrap();
                let invalid_a = uuid::Uuid::new_v4();
                let invalid_b = uuid::Uuid::new_v4();

                let err = ctx
                    .move_cards(vec![valid.id, invalid_a, invalid_b], dst_col.id)
                    .unwrap_err();
                assert!(
                    err.is_not_found(),
                    "expected NotFound (precedes WIP pre-check), got {err:?}"
                );
            }

            // KAN-428 followup: duplicate ids in the input must be deduplicated
            // before the WIP pre-check compares against the limit. compute_move_positions
            // emits one MoveCard per unique id, so the pre-check must use the
            // same post-dedup count — otherwise a caller passing [a, a, a] into
            // a WIP-limited column with room for the one real move gets a
            // false WipLimitExceeded.
            #[tokio::test(flavor = "multi_thread")]
            async fn test_move_cards_with_duplicate_ids_uses_deduped_count_for_wip_check() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let board = ctx.create_board("B".into(), Some("TST".into())).unwrap();
                let src_col = ctx.create_column(board.id, "Src".into(), None).unwrap();
                let dst_col = ctx.create_column(board.id, "Dst".into(), None).unwrap();
                // Limit dst to 1; with empty dst and one unique mover (a),
                // [a, a, a] would have tripped the pre-check before the fix
                // (`ids.len() == 3 > 1`).
                ctx.update_column(
                    dst_col.id,
                    kanban_domain::ColumnUpdate {
                        wip_limit: kanban_domain::FieldUpdate::Set(1),
                        ..Default::default()
                    },
                )
                .unwrap();
                let card_a = ctx
                    .create_card(board.id, src_col.id, "A".into(), Default::default())
                    .unwrap();

                let count = ctx
                    .move_cards(vec![card_a.id, card_a.id, card_a.id], dst_col.id)
                    .expect("duplicate ids must dedup to one real move that fits the limit");
                assert_eq!(count, 1, "exactly one card should land in dst_col");

                let moved = backend.get_card(card_a.id).unwrap().unwrap();
                assert_eq!(moved.column_id, dst_col.id);
            }

            // move_cards_impl must dedup ids before computing chained status
            // updates, mirroring the dedup compute_move_positions already does
            // for MoveCard commands. Moving a duplicated id into the board's
            // completion column must emit exactly ONE chained UpdateCard
            // (status -> Done) for that card, not one per occurrence.
            #[tokio::test(flavor = "multi_thread")]
            async fn test_move_cards_with_duplicate_ids_emits_one_chained_status_update() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let board = ctx.create_board("B".into(), Some("TST".into())).unwrap();
                let src_col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
                let dst_col = ctx.create_column(board.id, "Done".into(), None).unwrap();
                ctx.update_board(
                    board.id,
                    kanban_domain::BoardUpdate {
                        completion_column_ids: Some(vec![dst_col.id]),
                        ..Default::default()
                    },
                )
                .unwrap();
                let card = ctx
                    .create_card(board.id, src_col.id, "C".into(), Default::default())
                    .unwrap();

                let baseline = backend.batch_count().unwrap();
                ctx.move_cards(vec![card.id, card.id], dst_col.id).unwrap();

                let batches = backend.load_batches(baseline, baseline + 1).unwrap();
                assert_eq!(batches.len(), 1, "move_cards executes as one batch");
                let status_updates = batches[0]
                    .commands
                    .iter()
                    .filter(|c| {
                        matches!(
                            c,
                            Command::Card(CardCommand::Update(u))
                                if u.card_id == card.id && u.updates.status.is_some()
                        )
                    })
                    .count();
                assert_eq!(
                    status_updates, 1,
                    "duplicate input ids must not emit duplicate chained status-update commands"
                );

                let moved = backend.get_card(card.id).unwrap().unwrap();
                assert_eq!(moved.column_id, dst_col.id);
                assert_eq!(moved.status, kanban_domain::CardStatus::Done);
            }

            // KAN-428 followup: move_cards_detailed.succeeded must report the
            // post-dedup mover list, not the raw input. compute_move_positions
            // collapses duplicates, so reporting succeeded = [a, a, a] when only
            // one MoveCard ran is a caller-visibility bug.
            #[tokio::test(flavor = "multi_thread")]
            async fn test_move_cards_detailed_dedupes_succeeded_for_duplicate_ids() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let board = ctx.create_board("B".into(), Some("TST".into())).unwrap();
                let src_col = ctx.create_column(board.id, "Src".into(), None).unwrap();
                let dst_col = ctx.create_column(board.id, "Dst".into(), None).unwrap();
                let card_a = ctx
                    .create_card(board.id, src_col.id, "A".into(), Default::default())
                    .unwrap();

                let result =
                    ctx.move_cards_detailed(vec![card_a.id, card_a.id, card_a.id], dst_col.id);

                assert_eq!(result.succeeded, vec![card_a.id]);
                assert!(result.failed.is_empty());

                let moved = backend.get_card(card_a.id).unwrap().unwrap();
                assert_eq!(moved.column_id, dst_col.id);
            }

            // KAN-428 followup: move_cards_detailed.failed must also dedup
            // repeated invalid ids — symmetric to the succeeded-side dedup.
            // A caller passing [bogus, bogus, bogus] should see one failure
            // entry, not three copies of the same not_found error.
            #[tokio::test(flavor = "multi_thread")]
            async fn test_move_cards_detailed_dedupes_failed_for_duplicate_invalid_ids() {
                let (mut ctx, _dir) = $open_ctx.await;

                let board = ctx.create_board("B".into(), Some("TST".into())).unwrap();
                let dst_col = ctx.create_column(board.id, "Dst".into(), None).unwrap();
                let bogus = uuid::Uuid::new_v4();

                let result = ctx.move_cards_detailed(vec![bogus, bogus, bogus], dst_col.id);

                assert!(result.succeeded.is_empty());
                assert_eq!(result.failed.len(), 1);
                assert_eq!(result.failed[0].id, bogus);
            }

            // KAN-916 (O1-A): a `None`-position move appends past the FULL
            // (live + archived) set, so a card moved into a column that already
            // holds an archived sibling never collides with the archived
            // ordinal. Before the fix the append used the live-only count and
            // handed out a position that an archived card already occupied.
            #[tokio::test(flavor = "multi_thread")]
            async fn test_move_archived_card_positions_coherently() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                // Destination column holds a live card at 0 and an archived
                // card at 1 (the marker model keeps the archived card live in
                // place, so it still occupies ordinal 1).
                let board = ctx.create_board("B".into(), Some("TST".into())).unwrap();
                let src = ctx.create_column(board.id, "Src".into(), None).unwrap();
                let dst = ctx.create_column(board.id, "Dst".into(), None).unwrap();

                let live_in_dst = ctx
                    .create_card(board.id, dst.id, "LiveDst".into(), Default::default())
                    .unwrap();
                let archived_in_dst = ctx
                    .create_card(board.id, dst.id, "ArchDst".into(), Default::default())
                    .unwrap();
                ctx.archive_card(archived_in_dst.id).unwrap();

                assert_eq!(live_in_dst.position, 0);
                let archived_row = backend.get_card(archived_in_dst.id).unwrap().unwrap();
                assert_eq!(
                    archived_row.position, 1,
                    "archived card keeps its live ordinal (marker model)"
                );

                // Move a live card from Src into Dst with position None.
                let mover = ctx
                    .create_card(board.id, src.id, "Mover".into(), Default::default())
                    .unwrap();
                let moved = ctx.move_card(mover.id, dst.id, None).unwrap();

                // Coherent append: past BOTH the live (0) AND the archived (1)
                // ordinal → position 2, not the live-only-count 1 that would
                // collide with the archived card.
                assert_eq!(
                    moved.position, 2,
                    "moved card must append past live+archived, not collide at 1"
                );
                assert_eq!(moved.column_id, dst.id);
                assert_ne!(
                    moved.position, archived_row.position,
                    "moved card must not share the archived card's ordinal"
                );
            }

            // KAN-916 / D4: archive is a pure marker flip that leaves the live
            // card's column/position untouched, so archive → (move a sibling)
            // → restore is the identity over the archived card's column and
            // position. Reload from the backend so a persisted incoherent row
            // would be caught.
            #[tokio::test(flavor = "multi_thread")]
            async fn test_archive_then_move_sibling_then_restore_round_trips() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let board = ctx.create_board("B".into(), Some("TST".into())).unwrap();
                let src = ctx.create_column(board.id, "Src".into(), None).unwrap();
                let dst = ctx.create_column(board.id, "Dst".into(), None).unwrap();

                // Column holds two live cards; card_a (ordinal 0) will be
                // archived, sibling card_b (ordinal 1) will be moved.
                let card_a = ctx
                    .create_card(board.id, dst.id, "A".into(), Default::default())
                    .unwrap();
                let card_b = ctx
                    .create_card(board.id, dst.id, "B".into(), Default::default())
                    .unwrap();
                let a_col_before = card_a.column_id;
                let a_pos_before = card_a.position;

                ctx.archive_card(card_a.id).unwrap();

                // Move the sibling out and back with None positions; each append
                // is now Include-aware so it accounts for the archived card_a.
                ctx.move_card(card_b.id, src.id, None).unwrap();
                ctx.move_card(card_b.id, dst.id, None).unwrap();

                // Restore card_a to its original (still-present) column.
                let restored = ctx.restore_card(card_a.id, None).unwrap();
                assert_eq!(
                    restored.column_id, a_col_before,
                    "restore is identity on column"
                );
                assert_eq!(
                    restored.position, a_pos_before,
                    "restore is identity on position"
                );

                // The persisted row agrees (reload from backend).
                let reloaded = backend.get_card(card_a.id).unwrap().unwrap();
                assert_eq!(reloaded.column_id, a_col_before);
                assert_eq!(reloaded.position, a_pos_before);
                assert!(
                    backend.get_archived_card(card_a.id).unwrap().is_none(),
                    "restore clears the archive marker"
                );
            }
        }
    };
}

move_cards_tests!(json_backend, open_json_ctx());
move_cards_tests!(sqlite_backend, open_sqlite_ctx());
