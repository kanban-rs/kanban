//! Service-layer contract: `list_archived_cards_sorted` mirrors
//! `list_cards`' sort behaviour — board defaults apply, explicit
//! overrides win, and an empty filter preserves storage order.

use chrono::{DateTime, Utc};
use kanban_core::AppConfig;
use kanban_domain::commands::{
    BoardCommand, CardCommand, ColumnCommand, Command, CreateBoard, CreateCard, CreateColumn,
    SetBoardTaskSort, UpdateCard,
};
use kanban_domain::{
    ArchivedCardListFilter, CardUpdate, CreateCardOptions, FieldUpdate, InMemoryStore,
    KanbanOperations, KanbanResult, SortField, SortOrder,
};
use kanban_service::KanbanContext;
use std::sync::Arc;
use uuid::Uuid;

async fn make_ctx() -> KanbanContext {
    KanbanContext::open(Arc::new(InMemoryStore::new()), AppConfig::default())
        .await
        .unwrap()
}

fn dt(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
}

async fn seed_three_archived_with_due_dates(
    ctx: &mut KanbanContext,
) -> KanbanResult<(Uuid, Uuid, Uuid, Uuid)> {
    let board_id = Uuid::new_v4();
    ctx.execute(vec![Command::Board(BoardCommand::Create(CreateBoard {
        id: board_id,
        name: "B".into(),
        card_prefix: None,
        position: 0,
    }))])?;

    let column_id = Uuid::new_v4();
    ctx.execute(vec![Command::Column(ColumnCommand::Create(CreateColumn {
        id: column_id,
        board_id,
        name: "Todo".into(),
        position: 0,
    }))])?;

    let mut ids = Vec::new();
    for i in 0..3 {
        let id = Uuid::new_v4();
        ctx.execute(vec![Command::Card(CardCommand::Create(CreateCard {
            id,
            card_number: (i as u32) + 1,
            board_id,
            column_id,
            title: format!("card-{}", i),
            position: i,
            options: CreateCardOptions::default(),
            timestamp: chrono::Utc::now(),
        }))])?;
        ids.push(id);
    }

    let due_dates = [
        (ids[2], dt("2026-01-01T00:00:00Z")),
        (ids[0], dt("2026-06-01T00:00:00Z")),
        (ids[1], dt("2026-12-01T00:00:00Z")),
    ];
    for (id, when) in due_dates {
        ctx.execute(vec![Command::Card(CardCommand::Update(UpdateCard {
            card_id: id,
            updates: CardUpdate {
                due_date: FieldUpdate::Set(when),
                ..Default::default()
            },
        }))])?;
    }

    for id in &ids {
        ctx.archive_card(*id)?;
    }

    Ok((board_id, ids[2], ids[0], ids[1]))
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_archived_cards_sorted_uses_board_default() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let (board_id, earliest, middle, latest) = seed_three_archived_with_due_dates(&mut ctx).await?;

    ctx.execute(vec![Command::Board(BoardCommand::SetTaskSort(
        SetBoardTaskSort {
            board_id,
            field: SortField::DueDate,
            order: SortOrder::Ascending,
        },
    ))])?;

    let archived = ctx.list_archived_cards_sorted(ArchivedCardListFilter {
        board_id: Some(board_id),
        ..Default::default()
    })?;

    let ids: Vec<Uuid> = archived.iter().map(|(card, _)| card.id).collect();
    assert_eq!(ids, vec![earliest, middle, latest]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_archived_cards_sorted_explicit_override_wins() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let (board_id, earliest, middle, latest) = seed_three_archived_with_due_dates(&mut ctx).await?;

    // Board default left at SortField::Default — override forces DueDate.
    let archived = ctx.list_archived_cards_sorted(ArchivedCardListFilter {
        board_id: Some(board_id),
        sort: Some(SortField::DueDate),
        sort_order: Some(SortOrder::Descending),
    })?;

    let ids: Vec<Uuid> = archived.iter().map(|(card, _)| card.id).collect();
    assert_eq!(ids, vec![latest, middle, earliest]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_archived_cards_sorted_filters_by_board() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let (board_a, _, _, _) = seed_three_archived_with_due_dates(&mut ctx).await?;
    let (_board_b, _, _, _) = seed_three_archived_with_due_dates(&mut ctx).await?;

    let archived = ctx.list_archived_cards_sorted(ArchivedCardListFilter {
        board_id: Some(board_a),
        ..Default::default()
    })?;

    assert_eq!(
        archived.len(),
        3,
        "board filter must scope archives to the target board only"
    );
    Ok(())
}
