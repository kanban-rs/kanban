//! KAN-1821: parity tests for the service-layer intent methods the TUI now
//! depends on directly (`delete_cards`, `swap_column_positions`) and the
//! corrected local branch of the pre-existing `archive_cards` (which now also
//! runs `CompactColumnPositions` for each affected column as part of the same
//! undoable batch, the step that previously had to be hand-rolled by the TUI in
//! `animation_tick`).
//!
//! Every test runs twice against real, persisting local backends (JSON and
//! SQLite — the same pattern `kanban-service/tests/move_cards.rs` uses) and
//! once against a remote-shaped backend (`kanban_service::test_helpers::remote_backend`
//! over a `RecordingRemoteWrites` mock that records, in order, the exact wire
//! calls it receives). For each flow we assert the whole entity graph (board,
//! columns, cards) is present/absent as expected, not just the root object
//! the operation names, per this repo's mandatory full-graph per-backend rule.
use kanban_backend::{RemoteBatchWrites, RemoteWrites};
use kanban_domain::{
    Board, Card, Column, Invalidation, KanbanOperations, NewBoard, NewColumn, UndoOperations,
};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_persistence_sqlite::SqliteBackend;
use kanban_service::test_helpers::remote_backend;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;
use tempfile::tempdir;
use uuid::Uuid;

// ------------------------------------------------------------------
// Recording RemoteWrites mock: records which wire calls were made, in order.
// ------------------------------------------------------------------

#[derive(Default)]
struct RecordingRemoteWrites {
    calls: Arc<std::sync::Mutex<Vec<String>>>,
    board_id: Arc<std::sync::Mutex<Option<Uuid>>>,
}

impl RecordingRemoteWrites {
    fn new() -> Self {
        Self::default()
    }

    fn record(&self, name: &str) {
        self.calls.lock().unwrap().push(name.to_string());
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl RemoteWrites for RecordingRemoteWrites {
    fn create_board(
        &self,
        _id: Option<Uuid>,
        _spec: &NewBoard,
    ) -> kanban_domain::KanbanResult<(Board, Invalidation)> {
        self.record("create_board");
        let board = Board::new("remote-board", Some("TST"));
        *self.board_id.lock().unwrap() = Some(board.id);
        Ok((board, Invalidation::All))
    }

    fn update_board(
        &self,
        _id: Uuid,
        _updates: &kanban_domain::BoardUpdate,
    ) -> kanban_domain::KanbanResult<(Board, Invalidation)> {
        unimplemented!("update_board not needed by the flows under test")
    }

    fn delete_board(&self, _id: Uuid) -> kanban_domain::KanbanResult<Invalidation> {
        unimplemented!("delete_board not needed by the flows under test")
    }

    fn create_column(
        &self,
        board_id: Uuid,
        _spec: &NewColumn,
    ) -> kanban_domain::KanbanResult<(Column, Invalidation)> {
        self.record("create_column");
        let column = Column::new(board_id, "remote-column", 0);
        Ok((column, Invalidation::All))
    }

    fn update_column(
        &self,
        id: Uuid,
        updates: &kanban_domain::ColumnUpdate,
    ) -> kanban_domain::KanbanResult<(Column, Invalidation)> {
        self.record("update_column");
        let mut column = Column::new(Uuid::new_v4(), "remote-column", 0);
        column.id = id;
        if let Some(position) = updates.position {
            column.position = position;
        }
        Ok((column, Invalidation::All))
    }

    fn delete_column(&self, _id: Uuid) -> kanban_domain::KanbanResult<Invalidation> {
        unimplemented!("delete_column not needed by the flows under test")
    }

    fn create_card(
        &self,
        _id: Option<Uuid>,
        _spec: &kanban_domain::NewCard,
    ) -> kanban_domain::KanbanResult<(Card, Invalidation)> {
        unimplemented!("create_card not needed by the flows under test")
    }

    fn update_card(
        &self,
        _id: Uuid,
        _updates: &kanban_domain::CardUpdate,
    ) -> kanban_domain::KanbanResult<(Card, Invalidation)> {
        unimplemented!("update_card not needed by the flows under test")
    }

    fn delete_card(&self, _id: Uuid) -> kanban_domain::KanbanResult<Invalidation> {
        self.record("delete_card");
        Ok(Invalidation::All)
    }
}

impl RemoteBatchWrites for RecordingRemoteWrites {
    fn archive_cards(
        &self,
        _ids: &[Uuid],
    ) -> kanban_domain::KanbanResult<(kanban_backend::RemoteBatchOutcome, Invalidation)> {
        self.record("archive_cards");
        let outcome = kanban_backend::RemoteBatchOutcome {
            succeeded: Vec::new(),
            failed: Vec::new(),
        };
        Ok((outcome, Invalidation::All))
    }

    fn move_cards(
        &self,
        _ids: &[Uuid],
        _column_id: Uuid,
    ) -> kanban_domain::KanbanResult<(kanban_backend::RemoteBatchOutcome, Invalidation)> {
        unimplemented!("move_cards not needed by the flows under test")
    }

    fn assign_cards_to_sprint(
        &self,
        _ids: &[Uuid],
        _sprint_id: Uuid,
    ) -> kanban_domain::KanbanResult<(kanban_backend::RemoteBatchOutcome, Invalidation)> {
        unimplemented!("assign_cards_to_sprint not needed by the flows under test")
    }

    fn update_cards(
        &self,
        _updates: &[(Uuid, kanban_domain::CardUpdate)],
    ) -> kanban_domain::KanbanResult<(kanban_backend::RemoteBatchOutcome, Invalidation)> {
        unimplemented!("update_cards not needed by the flows under test")
    }
}

// ------------------------------------------------------------------
// Local-backend helpers (JSON + SQLite, real persistence), mirroring the
// `open_json_ctx`/`open_sqlite_ctx` pattern established in
// `kanban-service/tests/move_cards.rs`.
// ------------------------------------------------------------------

macro_rules! local_contexts {
    () => {
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
    };
}

local_contexts!();

/// Seed the standard "whole graph" fixture the card's acceptance criteria
/// require (a non-trivial board → columns → cards graph), and return ids the
/// per-flow tests need.
async fn seed_graph(ctx: &mut KanbanContext) -> (Board, Column, Column, Card, Card, Card) {
    let board = ctx.create_board("B".into(), Some("TST".into())).unwrap();
    let col_a = ctx.create_column(board.id, "A".into(), Some(0)).unwrap();
    let col_b = ctx.create_column(board.id, "B".into(), Some(1)).unwrap();
    let card_a1 = ctx
        .create_card(board.id, col_a.id, "A1".into(), Default::default())
        .unwrap();
    let card_a2 = ctx
        .create_card(board.id, col_a.id, "A2".into(), Default::default())
        .unwrap();
    let card_b1 = ctx
        .create_card(board.id, col_b.id, "B1".into(), Default::default())
        .unwrap();
    (board, col_a, col_b, card_a1, card_a2, card_b1)
}

// ------------------------------------------------------------------
// delete_cards
// ------------------------------------------------------------------

macro_rules! delete_cards_tests {
    ($mod_name:ident, $open_ctx:expr) => {
        mod $mod_name {
            use super::*;

            #[tokio::test(flavor = "multi_thread")]
            async fn test_delete_cards_local_removes_every_card_not_collateral() {
                let (mut ctx, _dir) = $open_ctx.await;
                let (board, col_a, _col_b, card_a1, card_a2, card_b1) = seed_graph(&mut ctx).await;
                let backend = ctx.backend();

                let count = ctx
                    .delete_cards(vec![card_a1.id, card_a2.id, card_b1.id])
                    .unwrap();
                assert_eq!(count, 3);

                for id in [card_a1.id, card_a2.id, card_b1.id] {
                    assert!(
                        backend.get_card(id).unwrap().is_none(),
                        "card {id} should be gone after delete_cards"
                    );
                }
                // The columns and board themselves must be untouched — only the
                // named cards may be removed, never collateral.
                assert!(backend.get_column(col_a.id).unwrap().is_some());
                assert!(backend.get_board(board.id).unwrap().is_some());
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_delete_cards_local_is_one_undo_entry() {
                let (mut ctx, _dir) = $open_ctx.await;
                let (_board, _col_a, _col_b, card_a1, card_a2, card_b1) =
                    seed_graph(&mut ctx).await;

                ctx.delete_cards(vec![card_a1.id, card_a2.id, card_b1.id])
                    .unwrap();

                // One action, one undo step — undo restores the whole deleted
                // set together, proving the batch was captured as a single unit
                // rather than N independent ones.
                let undone = ctx.undo().unwrap();
                assert!(undone.is_some(), "a single undo must restore the batch");
                let backend = ctx.backend();
                for id in [card_a1.id, card_a2.id, card_b1.id] {
                    assert!(
                        backend.get_card(id).unwrap().is_some(),
                        "card {id} must come back after undoing the single batch step"
                    );
                }
            }
        }
    };
}

delete_cards_tests!(delete_cards_json, open_json_ctx());
delete_cards_tests!(delete_cards_sqlite, open_sqlite_ctx());

#[tokio::test]
async fn test_delete_cards_remote_issues_one_wire_call_per_card_in_order() {
    let mock = Arc::new(RecordingRemoteWrites::new());
    let backend: Arc<dyn KanbanBackend> =
        remote_backend(Arc::clone(&mock) as Arc<dyn RemoteWrites>);
    let mut ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();

    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    let count = ctx.delete_cards(vec![id1, id2]).unwrap();
    assert_eq!(count, 2);

    let calls = mock.calls();
    assert_eq!(
        calls,
        vec!["delete_card".to_string(), "delete_card".to_string()],
        "each card must map to exactly one remote `delete_card` wire call"
    );
}

// ------------------------------------------------------------------
// swap_column_positions
// ------------------------------------------------------------------

macro_rules! swap_column_positions_tests {
    ($mod_name:ident, $open_ctx:expr) => {
        mod $mod_name {
            use super::*;

            #[tokio::test(flavor = "multi_thread")]
            async fn test_swap_column_positions_local_exchanges_exactly_their_two_positions() {
                let (mut ctx, _dir) = $open_ctx.await;
                let (_board, col_a, col_b, card_a1, _card_a2, card_b1) = seed_graph(&mut ctx).await;
                let backend = ctx.backend();
                let a1_pos_before = backend.get_card(card_a1.id).unwrap().unwrap().position;
                let b1_pos_before = backend.get_card(card_b1.id).unwrap().unwrap().position;

                let (first_after, second_after) =
                    ctx.swap_column_positions(col_a.id, col_b.id, 0, 1).unwrap();

                assert_eq!(first_after.id, col_a.id);
                assert_eq!(second_after.id, col_b.id);
                assert_eq!(
                    first_after.position, 1,
                    "A (was 0) should now hold B's former position"
                );
                assert_eq!(
                    second_after.position, 0,
                    "B (was 1) should now hold A's former position"
                );

                // The cards' own positions and column membership must be untouched
                // — only the two columns' ordering changed.
                let a1_pos_after = backend.get_card(card_a1.id).unwrap().unwrap().position;
                let b1_pos_after = backend.get_card(card_b1.id).unwrap().unwrap().position;
                assert_eq!(a1_pos_after, a1_pos_before);
                assert_eq!(b1_pos_after, b1_pos_before);
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_swap_column_positions_local_is_one_undo_entry() {
                let (mut ctx, _dir) = $open_ctx.await;
                let (_board, col_a, col_b, _card_a1, _card_a2, _card_b1) =
                    seed_graph(&mut ctx).await;
                let backend = ctx.backend();

                ctx.swap_column_positions(col_a.id, col_b.id, 0, 1).unwrap();

                let undone = ctx.undo().unwrap();
                assert!(undone.is_some(), "a swap must be a single undoable step");
                assert_eq!(
                    backend.get_column(col_a.id).unwrap().unwrap().position,
                    0,
                    "undoing the swap must restore A's original position"
                );
                assert_eq!(
                    backend.get_column(col_b.id).unwrap().unwrap().position,
                    1,
                    "undoing the swap must restore B's original position"
                );
            }
        }
    };
}

swap_column_positions_tests!(swap_columns_json, open_json_ctx());
swap_column_positions_tests!(swap_columns_sqlite, open_sqlite_ctx());

#[tokio::test]
async fn test_swap_column_positions_remote_issues_two_update_column_calls_in_argument_order() {
    let mock = Arc::new(RecordingRemoteWrites::new());
    let backend: Arc<dyn KanbanBackend> =
        remote_backend(Arc::clone(&mock) as Arc<dyn RemoteWrites>);
    let mut ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();

    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    ctx.swap_column_positions(first, second, 3, 7).unwrap();

    let calls = mock.calls();
    assert_eq!(
        calls,
        vec!["update_column".to_string(), "update_column".to_string()],
        "one update_column wire call per column, in argument order"
    );
}

// ------------------------------------------------------------------
// archive_cards: the local branch now compacts each affected column as part
// of the same undoable batch
// ------------------------------------------------------------------

macro_rules! archive_cards_compaction_tests {
    ($mod_name:ident, $open_ctx:expr) => {
        mod $mod_name {
            use super::*;

            #[tokio::test(flavor = "multi_thread")]
            async fn test_archive_cards_local_repacts_positions_of_each_affected_column() {
                let (mut ctx, _dir) = $open_ctx.await;
                let (_board, _col_a, _col_b, card_a1, card_a2, _card_b1) =
                    seed_graph(&mut ctx).await;
                let backend = ctx.backend();

                // col_a holds card_a1 (pos 0) and card_a2 (pos 1). Archiving only
                // card_a1 used to leave card_a2 at position 1 with a hole at 0.
                // `archive_cards` now folds the compaction step into the same
                // batch, so card_a2 must land at position 0.
                ctx.archive_cards(vec![card_a1.id]).unwrap();

                let card_a2_after = backend.get_card(card_a2.id).unwrap().unwrap();
                assert_eq!(
                    card_a2_after.position, 0,
                    "card_a2's position must be repacked to 0 after card_a1 is archived"
                );
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_archive_cards_local_repact_is_part_of_the_single_undo_entry() {
                let (mut ctx, _dir) = $open_ctx.await;
                let (_board, _col_a, _col_b, card_a1, card_a2, _card_b1) =
                    seed_graph(&mut ctx).await;
                let backend = ctx.backend();
                ctx.archive_cards(vec![card_a1.id]).unwrap();
                assert_eq!(
                    backend.get_card(card_a2.id).unwrap().unwrap().position,
                    0,
                    "precondition: compaction already applied"
                );

                let undone = ctx.undo().unwrap();
                assert!(undone.is_some());
                assert_eq!(
                    backend.get_card(card_a1.id).unwrap().unwrap().position,
                    0,
                    "undoing the single batch step must restore card_a1's original position"
                );
                assert_eq!(
                    backend.get_card(card_a2.id).unwrap().unwrap().position,
                    1,
                    "and card_a2's original (pre-compaction) position alongside it"
                );
            }
        }
    };
}

archive_cards_compaction_tests!(archive_compaction_json, open_json_ctx());
archive_cards_compaction_tests!(archive_compaction_sqlite, open_sqlite_ctx());

#[tokio::test]
async fn test_archive_cards_remote_issues_only_the_batch_wire_call_no_compaction_side_calls() {
    // KAN-1821 design: the `CompactColumnPositions` step is local-only and
    // deliberately absent from `RemoteWrites` — a remote backend computes its
    // own next position robustly to gaps, so the repack has no wire form. The
    // remote branch of `archive_cards_impl` must therefore issue exactly one
    // batch call and nothing else. `archive_cards` routes through
    // `KanbanBackend::remote_batch_writes()` (the dedicated batch surface), so
    // this must exercise that surface, not the single-entity `RemoteWrites`
    // surface `remote_backend` wires up.
    let mock = Arc::new(RecordingRemoteWrites::new());
    let backend: Arc<dyn KanbanBackend> = kanban_service::test_helpers::remote_batch_backend(
        Arc::clone(&mock) as Arc<dyn RemoteWrites>,
        Arc::clone(&mock) as Arc<dyn RemoteBatchWrites>,
    );
    let mut ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();

    let id = Uuid::new_v4();
    ctx.archive_cards(vec![id]).unwrap();

    let calls = mock.calls();
    assert_eq!(
        calls,
        vec!["archive_cards".to_string()],
        "the remote branch must issue exactly the single batch call, no compaction side calls"
    );
}
