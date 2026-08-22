use std::sync::Arc;

use kanban_core::AppConfig;
use kanban_domain::{
    requestable, Board, BoardUpdate, Card, Column, DataStore, EntityIds, FetchPlan, FetchRound,
    Invalidation, KanbanOperations, LoadedState,
};
use uuid::Uuid;

use crate::read_recorder::{assert_ops, ReadOp, RecordingStore};
use crate::{KanbanBackend, KanbanContext};

struct BoardListPlan;

impl FetchPlan for BoardListPlan {
    fn next_round(&self, loaded: &dyn LoadedState) -> FetchRound {
        FetchRound {
            board_list: requestable(loaded.board_list()),
            ..Default::default()
        }
    }
}

struct CardsByIdPlan {
    ids: Vec<Uuid>,
}

impl FetchPlan for CardsByIdPlan {
    fn next_round(&self, loaded: &dyn LoadedState) -> FetchRound {
        FetchRound {
            cards: self
                .ids
                .iter()
                .copied()
                .filter(|&id| requestable(loaded.card(id)))
                .collect(),
            ..Default::default()
        }
    }
}

fn ctx_over(store: &Arc<RecordingStore>) -> KanbanContext {
    let backend: Arc<dyn KanbanBackend> = Arc::clone(store) as Arc<dyn KanbanBackend>;
    KanbanContext::open_deferred(backend, AppConfig::default())
}

#[test]
fn test_a_default_context_has_no_cache() {
    let store = Arc::new(RecordingStore::new());
    let ctx = ctx_over(&store);

    assert!(!ctx.has_cache());
}

#[test]
fn test_opting_in_enables_the_cache() {
    let store = Arc::new(RecordingStore::new());
    let ctx = ctx_over(&store);
    assert!(!ctx.has_cache());

    let ctx = ctx.with_entity_cache();

    assert!(ctx.has_cache());
}

#[test]
fn test_resolve_on_a_cacheless_context_returns_default_resolved() {
    let store = Arc::new(RecordingStore::new());
    let board = Board::new("board", None::<String>);
    store.upsert_board(board).unwrap();
    store.clear_log();
    let mut ctx = ctx_over(&store);

    let resolved = ctx.resolve(&BoardListPlan).unwrap();

    assert!(resolved.boards.is_untouched());
    assert!(resolved.columns.is_untouched());
    assert!(resolved.cards.is_untouched());
    assert!(resolved.sprints.is_untouched());
    assert!(resolved.graph.is_not_loaded());
    assert_ops(&store.ops(), &[]);
}

#[test]
fn test_resolve_after_invalidate_all_reads_the_backend() {
    let store = Arc::new(RecordingStore::new());
    let board = Board::new("board", None::<String>);
    store.upsert_board(board).unwrap();
    store.clear_log();
    let mut ctx = ctx_over(&store).with_entity_cache();

    ctx.resolve(&BoardListPlan).unwrap();
    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "list_boards",
            ids: vec![],
        }],
    );

    store.clear_log();
    ctx.resolve(&BoardListPlan).unwrap();
    assert_ops(&store.ops(), &[]);

    ctx.invalidate(Invalidation::All);
    store.clear_log();
    let resolved = ctx.resolve(&BoardListPlan).unwrap();
    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "list_boards",
            ids: vec![],
        }],
    );
    assert!(resolved.boards.all.is_loaded());
}

#[test]
fn test_resolve_with_nothing_invalidated_reads_nothing() {
    let store = Arc::new(RecordingStore::new());
    let board = Board::new("board", None::<String>);
    store.upsert_board(board).unwrap();
    store.clear_log();
    let mut ctx = ctx_over(&store).with_entity_cache();

    ctx.resolve(&BoardListPlan).unwrap();
    store.clear_log();

    let resolved = ctx.resolve(&BoardListPlan).unwrap();

    assert_ops(&store.ops(), &[]);
    assert!(resolved.boards.is_untouched());
}

#[test]
fn test_invalidating_entities_re_reads_only_those_ids() {
    let store = Arc::new(RecordingStore::new());
    let board = Board::new("board", None::<String>);
    let board_id = board.id;
    store.upsert_board(board).unwrap();
    let column = Column::new(board_id, "col", 0);
    let column_id = column.id;
    store.upsert_column(column).unwrap();
    let card_a = Card::new(board_id, column_id, "a", 0);
    let card_b = Card::new(board_id, column_id, "b", 0);
    let a_id = card_a.id;
    let b_id = card_b.id;
    store.upsert_card(card_a).unwrap();
    store.upsert_card(card_b).unwrap();
    store.clear_log();
    let mut ctx = ctx_over(&store).with_entity_cache();
    let plan = CardsByIdPlan {
        ids: vec![a_id, b_id],
    };

    ctx.resolve(&plan).unwrap();
    assert_ops(
        &store.ops(),
        &[
            ReadOp {
                method: "get_card",
                ids: vec![a_id],
            },
            ReadOp {
                method: "get_card",
                ids: vec![b_id],
            },
        ],
    );

    store.clear_log();
    ctx.invalidate(Invalidation::Entities(EntityIds::cards([a_id])));
    ctx.resolve(&plan).unwrap();

    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "get_card",
            ids: vec![a_id],
        }],
    );
}

#[test]
fn test_command_execution_invalidates_the_cache() {
    let store = Arc::new(RecordingStore::new());
    let board = Board::new("board", None::<String>);
    let board_id = board.id;
    store.upsert_board(board.clone()).unwrap();
    store.clear_log();
    let mut ctx = ctx_over(&store).with_entity_cache();

    ctx.resolve(&BoardListPlan).unwrap();

    store.clear_log();
    ctx.resolve(&BoardListPlan).unwrap();
    assert_ops(&store.ops(), &[]);

    ctx.update_board(
        board_id,
        BoardUpdate {
            name: Some("renamed".into()),
            ..Default::default()
        },
    )
    .unwrap();

    store.clear_log();
    let resolved = ctx.resolve(&BoardListPlan).unwrap();

    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "list_boards",
            ids: vec![],
        }],
    );
    assert_eq!(
        resolved.boards.all.loaded().unwrap()[0].name,
        "renamed".to_string()
    );
}
