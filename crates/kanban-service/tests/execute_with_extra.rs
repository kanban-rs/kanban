use kanban_backend_memory::InMemoryStore;
use kanban_domain::commands::{CardCommand, Command, CreateCard, UpdateCard};
use kanban_domain::{
    CardUpdate, CreateCardOptions, EntityIds, Invalidation, KanbanOperations, KanbanResult,
};
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

fn entities(inv: Invalidation) -> EntityIds {
    match inv {
        Invalidation::Entities(ids) => ids,
        Invalidation::All => panic!("expected Entities, got All"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_execute_with_extra_merges_the_extra_into_the_derived_entities() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;

    let inv = ctx.execute_with_extra(EntityIds::default().with_prefixes(), |_| {
        Ok(vec![Command::Card(CardCommand::Update(UpdateCard {
            card_id: card.id,
            updates: CardUpdate {
                title: Some("Renamed".into()),
                ..Default::default()
            },
        }))])
    })?;

    let ids = entities(inv);
    assert_eq!(ids.cards, HashSet::from([card.id]));
    assert!(ids.prefixes);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_execute_with_extra_does_not_downgrade_all_to_entities() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;

    let inv = ctx.execute_with_extra(EntityIds::default().with_prefixes(), |_| {
        Ok(vec![Command::Card(CardCommand::Create(CreateCard {
            id: Uuid::new_v4(),
            card_number: 1,
            board_id: board.id,
            column_id: col.id,
            title: "c".into(),
            position: 0,
            options: CreateCardOptions::default(),
            timestamp: chrono::Utc::now(),
            default_card_prefix: "task".into(),
        }))])
    })?;

    assert_eq!(inv, Invalidation::All);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_plain_execute_with_records_no_prefixes() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;

    let inv = ctx.execute_with(|_| {
        Ok(vec![Command::Card(CardCommand::Update(UpdateCard {
            card_id: card.id,
            updates: CardUpdate {
                title: Some("Renamed".into()),
                ..Default::default()
            },
        }))])
    })?;

    let ids = entities(inv);
    assert_eq!(ids.cards, HashSet::from([card.id]));
    assert!(!ids.prefixes);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_service_create_card_records_an_invalidation_covering_prefixes() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;

    let (_card, inv) = ctx.create_card_impl(board.id, col.id, "A".into(), Default::default())?;

    let covers_prefixes = match inv {
        Invalidation::All => true,
        Invalidation::Entities(ids) => ids.prefixes,
    };
    assert!(covers_prefixes);
    Ok(())
}
