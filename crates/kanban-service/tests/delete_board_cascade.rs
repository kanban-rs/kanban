//! Integration tests for `KanbanContext::delete_board` cascade orchestration (KAN-427).
//!
//! Run against both `JsonDataStore` and `SqliteBackend` via a macro to catch any
//! backend-specific divergence in the cascade path.

use kanban_domain::{
    commands::{Command, MoveCard},
    ArchivedCard, Board, Card, Column, Sprint,
};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_persistence_sqlite::SqliteBackend;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext, KanbanOperations};
use std::sync::Arc;
use tempfile::tempdir;
use uuid::Uuid;

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

macro_rules! cascade_tests {
    ($mod_name:ident, $open_ctx:expr) => {
        mod $mod_name {
            use super::*;

            #[tokio::test(flavor = "multi_thread")]
            async fn test_delete_board_cascades_to_columns_cards_and_sprints() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let board = Board::new("B", Some("TST"));
                let board_id = board.id;
                let col1 = Column::new(board_id, "Col1", 0);
                let col2 = Column::new(board_id, "Col2", 1);
                let card1 = Card::new(board.id, col1.id, "C1", 0);
                let card2 = Card::new(board.id, col2.id, "C2", 0);
                let sprint = Sprint::new(board_id, 1, None, None::<String>);
                backend.upsert_board(board).unwrap();
                backend.upsert_column(col1).unwrap();
                backend.upsert_column(col2).unwrap();
                backend.upsert_card(card1).unwrap();
                backend.upsert_card(card2).unwrap();
                backend.upsert_sprint(sprint).unwrap();

                ctx.delete_board(board_id).unwrap();

                assert!(backend.list_boards().unwrap().is_empty());
                assert!(backend.list_all_columns().unwrap().is_empty());
                assert!(backend.list_all_cards().unwrap().is_empty());
                assert!(backend.list_all_sprints().unwrap().is_empty());
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_delete_board_cleans_dependency_graph_edges_for_all_cards() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let board = Board::new("B", Some("TST"));
                let board_id = board.id;
                let col = Column::new(board_id, "Col", 0);
                let card_a = Card::new(board.id, col.id, "A", 0);
                let card_b = Card::new(board.id, col.id, "B", 1);
                let card_a_id = card_a.id;
                let card_b_id = card_b.id;
                backend.upsert_board(board).unwrap();
                backend.upsert_column(col).unwrap();
                backend.upsert_card(card_a).unwrap();
                backend.upsert_card(card_b).unwrap();

                let mut graph = backend.get_graph().unwrap();
                graph.set_block(card_a_id, card_b_id).unwrap();
                backend.set_graph(graph).unwrap();
                assert_eq!(backend.get_graph().unwrap().len(), 1);

                ctx.delete_board(board_id).unwrap();

                assert_eq!(
                    backend.get_graph().unwrap().len(),
                    0,
                    "service delete_board must clean dependency-graph edges for all deleted cards"
                );
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_delete_board_removes_archived_cards() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let board = Board::new("B", Some("TST"));
                let board_id = board.id;
                let col = Column::new(board_id, "Col", 0);
                let col_id = col.id;
                let card = Card::new(board.id, col_id, "C", 0);
                let card_id = card.id;
                // Reference-marker model: the marker references a LIVE card by id
                // (FK `archived_cards.card_id -> cards.id`), so the card row must
                // exist before the marker is inserted.
                let archived = ArchivedCard::new(card_id, board_id);
                backend.upsert_board(board).unwrap();
                backend.upsert_column(col).unwrap();
                backend.upsert_card(card).unwrap();
                backend.insert_archived_card(archived).unwrap();

                ctx.delete_board(board_id).unwrap();

                assert!(backend.list_archived_cards().unwrap().is_empty());
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_delete_board_undo_restores_full_cascade_state() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                // A board carrying every entity the cascade touches: a column,
                // two live cards related by an edge, an archived card, and a
                // sprint.
                let board = ctx.create_board("B".into(), Some("TST".into())).unwrap();
                let board_id = board.id;
                let column = ctx.create_column(board_id, "Col".into(), None).unwrap();
                let card_a = ctx
                    .create_card(board_id, column.id, "A".into(), Default::default())
                    .unwrap();
                let card_b = ctx
                    .create_card(board_id, column.id, "B".into(), Default::default())
                    .unwrap();
                let sprint_id = ctx.create_sprint(board_id, None, None).unwrap().id;

                // A relation (block edge) between the two live cards.
                let mut graph = backend.get_graph().unwrap();
                graph.set_block(card_a.id, card_b.id).unwrap();
                backend.set_graph(graph).unwrap();

                // An archived card in the same column.
                let arch_board = board.clone();
                let arch_card = Card::new(arch_board.id, column.id, "C", 2);
                let arch_card_id = arch_card.id;
                // The marker references the live card by id, so seed the live row
                // first, then the marker over it.
                backend.upsert_card(arch_card).unwrap();
                backend
                    .insert_archived_card(ArchivedCard::new(arch_card_id, board_id))
                    .unwrap();

                assert_eq!(backend.list_boards().unwrap().len(), 1);
                assert_eq!(backend.list_all_cards().unwrap().len(), 2);
                assert_eq!(backend.list_archived_cards().unwrap().len(), 1);
                assert_eq!(backend.list_all_sprints().unwrap().len(), 1);
                assert_eq!(backend.get_graph().unwrap().len(), 1);

                ctx.delete_board(board_id).unwrap();

                // Everything the board owned is gone, including the relation.
                assert!(backend.list_boards().unwrap().is_empty());
                assert!(backend.list_all_cards().unwrap().is_empty());
                assert!(backend.list_archived_cards().unwrap().is_empty());
                assert!(backend.list_all_sprints().unwrap().is_empty());
                assert_eq!(backend.get_graph().unwrap().len(), 0);

                let undone = ctx.undo().unwrap();
                assert!(undone, "undo should report success");

                // A single undo restores the ENTIRE cascade - the SAME entities
                // (by id/content), not just matching counts.
                assert_eq!(
                    backend.list_boards().unwrap().len(),
                    1,
                    "undo must restore the board"
                );
                assert_eq!(
                    backend.list_all_columns().unwrap().len(),
                    1,
                    "undo must restore the column"
                );

                let cards = backend.list_all_cards().unwrap();
                assert_eq!(cards.len(), 2, "exactly the two live cards return");
                let card_a_back = cards
                    .iter()
                    .find(|c| c.id == card_a.id)
                    .expect("live card A restored by id");
                assert_eq!(card_a_back.title, "A", "card A restored with its content");
                assert_eq!(
                    card_a_back.column_id, column.id,
                    "card A back in its column"
                );
                assert!(
                    cards.iter().any(|c| c.id == card_b.id),
                    "live card B restored by id"
                );

                // Exactly the one block edge we created (A -> B) comes back; the
                // delete cleaned it to 0 above, so len == 1 pins its restoration.
                assert_eq!(
                    backend.get_graph().unwrap().len(),
                    1,
                    "undo must restore the relation between the cards"
                );

                let archived = backend.list_archived_cards().unwrap();
                assert!(
                    archived.iter().any(|ac| ac.entity_id == arch_card_id),
                    "the deleted board's archived card is restored by id"
                );

                assert!(
                    backend
                        .list_all_sprints()
                        .unwrap()
                        .iter()
                        .any(|s| s.id == sprint_id),
                    "undo must restore the sprint by id"
                );
            }

            /// Drives a partial-failure batch through `execute(...)` — the same code path
            /// `delete_board` uses — and verifies snapshot rollback restores the pre-state
            /// and does not append the failed batch to the command log.
            #[tokio::test(flavor = "multi_thread")]
            async fn test_execute_batch_rolls_back_on_mid_batch_failure() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let board = ctx.create_board("B".into(), Some("TST".into())).unwrap();
                let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
                let card = ctx
                    .create_card(board.id, column.id, "C".into(), Default::default())
                    .unwrap();
                let original_title = card.title.clone();
                let original_column_id = card.column_id;

                let commands_before = backend.batch_count().unwrap();

                let bogus_card_id = Uuid::new_v4();
                let move_target = Uuid::new_v4();
                let result = ctx.execute(vec![
                    Command::Card(kanban_domain::commands::CardCommand::Move(MoveCard {
                        card_id: card.id,
                        new_column_id: column.id,
                        new_position: 5,
                    })),
                    Command::Card(kanban_domain::commands::CardCommand::Move(MoveCard {
                        card_id: bogus_card_id,
                        new_column_id: move_target,
                        new_position: 0,
                    })),
                ]);

                assert!(
                    result.is_err(),
                    "batch with non-existent card should fail mid-batch"
                );

                let restored = backend.get_card(card.id).unwrap().expect("card present");
                assert_eq!(
                    restored.title, original_title,
                    "rollback must restore the card to its pre-batch state"
                );
                assert_eq!(
                    restored.column_id, original_column_id,
                    "rollback must restore the card's column"
                );
                assert_eq!(
                    backend.batch_count().unwrap(),
                    commands_before,
                    "failed batch must not be appended to the command log"
                );
            }

            /// KAN-833 regression (B5, confirmed orphan): once the DeleteColumn
            /// archived-cards guard is gone, a card can be archived and then have
            /// its original column deleted. The board-delete cascade must still
            /// remove that archived record AND its dependency-graph edges. The old
            /// by-columns gather scoped off live columns, so a dangling
            /// `original_column_id` was missed and leaked. The fix gathers archived
            /// cards by the first-class `board_id` field (populated at archive time
            /// on both backends now that B4 has landed), so this passes on JSON and
            /// SQLite alike.
            #[tokio::test(flavor = "multi_thread")]
            async fn test_delete_board_removes_archived_cards_whose_column_was_deleted() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let board = ctx.create_board("B".into(), Some("TST".into())).unwrap();
                let board_id = board.id;
                let column = ctx.create_column(board_id, "X".into(), None).unwrap();
                let card_c = ctx
                    .create_card(board_id, column.id, "C".into(), Default::default())
                    .unwrap();
                let card_d = ctx
                    .create_card(board_id, column.id, "D".into(), Default::default())
                    .unwrap();

                // An edge between the two cards so the cascade has graph state to clean.
                let mut graph = backend.get_graph().unwrap();
                graph.set_block(card_c.id, card_d.id).unwrap();
                backend.set_graph(graph).unwrap();

                // Archive both (populates board_id = board_id, original_column_id =
                // column.id), then delete the column out from under them (now
                // permitted post-guard-removal).
                ctx.archive_card(card_c.id).unwrap();
                ctx.archive_card(card_d.id).unwrap();
                ctx.delete_column(column.id).unwrap();

                // The column is gone, but the archived records and their (archived)
                // edge remain.
                assert!(
                    backend.list_columns_by_board(board_id).unwrap().is_empty(),
                    "column X was deleted"
                );
                assert_eq!(
                    backend.list_archived_cards().unwrap().len(),
                    2,
                    "both archived records survive the column deletion"
                );
                assert!(
                    !backend.get_graph().unwrap().is_empty(),
                    "the edge between the archived cards is still present pre-cascade"
                );

                ctx.delete_board(board_id).unwrap();

                // The confirmed regression: with the by-columns cascade these leaked.
                assert!(
                    backend.list_archived_cards().unwrap().is_empty(),
                    "delete_board must remove archived records whose original column was deleted"
                );
                assert_eq!(
                    backend.get_graph().unwrap().len(),
                    0,
                    "delete_board must remove the dependency-graph edges of those archived cards"
                );
            }

            /// KAN-833 (B5 review fix): a board with sprints but NO columns and
            /// NO archived cards (reachable by deleting all empty columns) must
            /// still cascade `DeleteSprintsByBoard`. The old widened guard
            /// short-circuited to a bare `DeleteBoard`, leaking the sprints
            /// (JSON/in-memory) or FK-cascading them without undo capture
            /// (SQLite), so undo restored the board WITHOUT its sprint.
            #[tokio::test(flavor = "multi_thread")]
            async fn test_delete_board_with_only_sprints_cascades_and_undo_restores_sprint() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let board = ctx.create_board("B".into(), Some("TST".into())).unwrap();
                let board_id = board.id;
                let sprint_id = ctx.create_sprint(board_id, None, None).unwrap().id;

                assert!(
                    backend.list_columns_by_board(board_id).unwrap().is_empty(),
                    "board has no columns"
                );
                assert_eq!(
                    backend.list_all_sprints().unwrap().len(),
                    1,
                    "the sprint exists pre-delete"
                );

                ctx.delete_board(board_id).unwrap();

                assert!(
                    backend.list_all_sprints().unwrap().is_empty(),
                    "delete_board must remove the board's sprints even with no columns"
                );

                let undone = ctx.undo().unwrap();
                assert!(undone, "undo should report success");

                assert!(
                    backend
                        .list_all_sprints()
                        .unwrap()
                        .iter()
                        .any(|s| s.id == sprint_id),
                    "undo must restore the sprint by id"
                );
            }
        }
    };
}

cascade_tests!(json_backend, open_json_ctx());
cascade_tests!(sqlite_backend, open_sqlite_ctx());
