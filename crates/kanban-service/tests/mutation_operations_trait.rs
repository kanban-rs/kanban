use kanban_backend_memory::InMemoryStore;
use kanban_domain::commands::{BoardCommand, Command, UpdateBoard};
use kanban_domain::{
    BoardUpdate, EntityIds, Invalidation, KanbanOperations, MutationOperations, NewBoard,
};
use kanban_service::{BoardCreateOutcome, KanbanContext};
use std::sync::Arc;
use uuid::Uuid;

async fn make_ctx() -> KanbanContext {
    KanbanContext::open(
        Arc::new(InMemoryStore::new()),
        kanban_core::AppConfig::default(),
    )
    .await
    .unwrap()
}

fn board_spec(name: &str) -> NewBoard {
    NewBoard {
        name: name.to_string(),
        description: None,
        sprint_prefix: None,
        card_prefix: Some("KAN".into()),
        task_sort_field: None,
        task_sort_order: None,
        sprint_duration_days: None,
        task_list_view: None,
    }
}

#[tokio::test]
async fn test_mutation_operations_delegates_to_the_inherent_mutators() {
    let mut ctx = make_ctx().await;
    let (board, _inv) = ctx
        .create_board_from_spec(None, board_spec("Alpha"))
        .unwrap();

    let updates = BoardUpdate {
        name: Some("Renamed".into()),
        ..Default::default()
    };
    let inv = MutationOperations::execute(
        &mut ctx,
        vec![Command::Board(BoardCommand::Update(UpdateBoard {
            board_id: board.id,
            updates: updates.clone(),
        }))],
    )
    .unwrap();
    assert_eq!(inv, Invalidation::Entities(EntityIds::boards([board.id])));
    assert_eq!(ctx.get_board(board.id).unwrap().unwrap().name, "Renamed");

    let updates2 = BoardUpdate {
        name: Some("Renamed Again".into()),
        ..Default::default()
    };
    let (updated_board, tuple_inv) =
        MutationOperations::update_board_impl(&mut ctx, board.id, updates2).unwrap();
    assert_eq!(updated_board.name, "Renamed Again");
    assert_eq!(
        tuple_inv,
        Invalidation::Entities(EntityIds::boards([board.id]))
    );

    let unit_inv = MutationOperations::delete_board_impl(&mut ctx, board.id).unwrap();
    assert_eq!(
        unit_inv,
        Invalidation::Entities(EntityIds::boards([board.id]).with_prefixes())
    );
    assert!(ctx.get_board(board.id).unwrap().is_none());

    let fresh_id = Uuid::new_v4();
    let (outcome1, _inv) =
        MutationOperations::create_or_replace_board(&mut ctx, fresh_id, board_spec("Beta"))
            .unwrap();
    assert!(matches!(outcome1, BoardCreateOutcome { created: true, .. }));

    let (outcome2, _inv) =
        MutationOperations::create_or_replace_board(&mut ctx, fresh_id, board_spec("Beta v2"))
            .unwrap();
    assert!(matches!(
        outcome2,
        BoardCreateOutcome { created: false, .. }
    ));
}

#[tokio::test]
async fn test_mutation_operations_trait_object_reaches_the_store() {
    let mut ctx = make_ctx().await;
    let (board, _inv) = ctx
        .create_board_from_spec(None, board_spec("Gamma"))
        .unwrap();

    let ops: &mut dyn MutationOperations = &mut ctx;
    let (column, _inv) = ops
        .create_column_impl(board.id, "Todo".into(), None)
        .unwrap();
    let (parent, _inv) = ops
        .create_card_impl(
            board.id,
            column.id,
            "Parent".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let (child, _inv) = ops
        .create_card_impl(
            board.id,
            column.id,
            "Child".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let _ = ops.attach_children_impl(parent.id, vec![child.id]).unwrap();

    assert!(ctx.get_column(column.id).unwrap().is_some());
    let relations = ctx.list_relations_for_board(board.id).unwrap();
    assert_eq!(relations.spawns.len(), 1);
}
