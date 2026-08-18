use kanban_backend_memory::InMemoryStore;
use kanban_domain::commands::cascade_commands::{CascadeCommand, DeleteArchivedCards};
use kanban_domain::commands::{CardCommand, Command, CommandContext, MoveCard, UpdateCard};
use kanban_domain::data_store::DataStore;
use kanban_domain::{invalidation_from_inverse, CardUpdate, Invalidation, KanbanOperations};
use kanban_service::KanbanContext;
use std::collections::HashSet;
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

#[tokio::test(flavor = "multi_thread")]
async fn test_invalidation_from_a_real_capture_inverse_batch_names_exactly_the_touched_cards() {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None).unwrap();
    let col_a = ctx.create_column(board.id, "A".into(), None).unwrap();
    let col_b = ctx.create_column(board.id, "B".into(), None).unwrap();
    let card1 = ctx
        .create_card(board.id, col_a.id, "Card 1".into(), Default::default())
        .unwrap();
    let card2 = ctx
        .create_card(board.id, col_a.id, "Card 2".into(), Default::default())
        .unwrap();
    let _card3 = ctx
        .create_card(board.id, col_a.id, "Card 3".into(), Default::default())
        .unwrap();

    let backend = ctx.backend();
    let store: &dyn DataStore = backend.as_data_store();
    let cmd_ctx = CommandContext { store };

    let update_cmd = Command::Card(CardCommand::Update(UpdateCard {
        card_id: card1.id,
        updates: CardUpdate {
            title: Some("Renamed".into()),
            ..Default::default()
        },
    }));
    let mut inverse = update_cmd.capture_inverse(store).unwrap();
    update_cmd.execute(&cmd_ctx).unwrap();

    let move_cmd = Command::Card(CardCommand::Move(MoveCard {
        card_id: card2.id,
        new_column_id: col_b.id,
        new_position: 0,
    }));
    inverse.extend(move_cmd.capture_inverse(store).unwrap());
    move_cmd.execute(&cmd_ctx).unwrap();

    let invalidation = invalidation_from_inverse(&inverse);
    match invalidation {
        Invalidation::Entities(ids) => {
            assert_eq!(ids.cards, HashSet::from([card1.id, card2.id]));
        }
        Invalidation::All => panic!("expected Entities, got All"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_invalidation_from_a_real_empty_capture_inverse_is_all() {
    let ctx = make_ctx().await;
    let backend = ctx.backend();
    let store: &dyn DataStore = backend.as_data_store();

    let cmd = Command::Cascade(CascadeCommand::DeleteArchivedCards(DeleteArchivedCards {
        card_ids: vec![Uuid::new_v4()],
    }));
    let inverse = cmd.capture_inverse(store).unwrap();
    assert!(
        inverse.is_empty(),
        "no such archived card, inverse must be empty"
    );

    assert_eq!(invalidation_from_inverse(&inverse), Invalidation::All);
}
