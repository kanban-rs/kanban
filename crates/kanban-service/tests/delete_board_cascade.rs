//! Integration tests for `KanbanContext::delete_board` cascade orchestration (KAN-427).
//!
//! Run against both `JsonDataStore` and `SqliteBackend` via a macro to catch any
//! backend-specific divergence in the cascade path.

use kanban_domain::{
    commands::{Command, MoveCard},
    ArchivedCard, Board, Card, Column, Sprint,
};
use kanban_persistence_json::JsonFileStore;
use kanban_service::{
    json_backend::JsonDataStore, sqlite_backend::SqliteBackend, AppConfig, KanbanBackend,
    KanbanContext, KanbanOperations,
};
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

                let mut board = Board::new("B", Some("TST"));
                let board_id = board.id;
                let col1 = Column::new(board_id, "Col1", 0);
                let col2 = Column::new(board_id, "Col2", 1);
                let card1 = Card::new(&mut board, col1.id, "C1", 0);
                let card2 = Card::new(&mut board, col2.id, "C2", 0);
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

                let mut board = Board::new("B", Some("TST"));
                let board_id = board.id;
                let col = Column::new(board_id, "Col", 0);
                let card_a = Card::new(&mut board, col.id, "A", 0);
                let card_b = Card::new(&mut board, col.id, "B", 1);
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

                let mut board = Board::new("B", Some("TST"));
                let board_id = board.id;
                let col = Column::new(board_id, "Col", 0);
                let col_id = col.id;
                let card = Card::new(&mut board, col_id, "C", 0);
                let archived = ArchivedCard::new(card, col_id, 0);
                backend.upsert_board(board).unwrap();
                backend.upsert_column(col).unwrap();
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
                ctx.create_sprint(board_id, None, None).unwrap();

                // A relation (block edge) between the two live cards.
                let mut graph = backend.get_graph().unwrap();
                graph.set_block(card_a.id, card_b.id).unwrap();
                backend.set_graph(graph).unwrap();

                // An archived card in the same column.
                let mut arch_board = board.clone();
                let arch_card = Card::new(&mut arch_board, column.id, "C", 2);
                backend
                    .insert_archived_card(ArchivedCard::new(arch_card, column.id, 2))
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

                // A single undo restores the ENTIRE cascade.
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
                assert_eq!(
                    backend.list_all_cards().unwrap().len(),
                    2,
                    "undo must restore the live cards"
                );
                assert_eq!(
                    backend.get_graph().unwrap().len(),
                    1,
                    "undo must restore the relation/edge between the cards"
                );
                assert_eq!(
                    backend.list_archived_cards().unwrap().len(),
                    1,
                    "undo must restore the archived card"
                );
                assert_eq!(
                    backend.list_all_sprints().unwrap().len(),
                    1,
                    "undo must restore the sprint"
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
        }
    };
}

cascade_tests!(json_backend, open_json_ctx());
cascade_tests!(sqlite_backend, open_sqlite_ctx());
