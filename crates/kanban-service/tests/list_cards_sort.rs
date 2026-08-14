//! Service-layer contract: `list_cards` sorts results using the board's
//! `task_sort_field` (and `task_sort_order`) by default, and accepts an
//! explicit override via `CardListFilter`. This lifts sorting out of the
//! TUI so CLI and MCP inherit consistent ordering through a single source.

use chrono::{DateTime, Utc};
use kanban_backend_memory::InMemoryStore;
use kanban_core::AppConfig;
use kanban_domain::commands::{
    BoardCommand, CardCommand, ColumnCommand, Command, CreateBoard, CreateCard, CreateColumn,
    SetBoardTaskSort, UpdateCard,
};
use kanban_domain::{
    BoardUpdate, CardListFilter, CardUpdate, CreateCardOptions, FieldUpdate, KanbanOperations,
    KanbanResult, SortField, SortOrder,
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

/// Seed three cards on a single board/column with distinct due dates and
/// no other distinguishing sort fields. Returns the card ids in the order
/// (earliest_due, middle_due, latest_due) so callers can build expected
/// orderings without re-deriving them from due dates.
async fn seed_three_cards_with_due_dates(
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
        default_status: None,
    }))])?;

    let mut ids = Vec::new();
    for (i, _label) in ["a", "b", "c"].iter().enumerate() {
        let id = Uuid::new_v4();
        ctx.execute(vec![Command::Card(CardCommand::Create(CreateCard {
            id,
            card_number: (i as u32) + 1,
            board_id,
            column_id,
            title: format!("card-{}", i),
            position: i as i32,
            options: CreateCardOptions::default(),
            timestamp: chrono::Utc::now(),
        }))])?;
        ids.push(id);
    }

    // Assign due dates: ids[2]=earliest, ids[0]=middle, ids[1]=latest.
    // Deliberately not aligned with insertion order to prove the sort
    // ran rather than the storage order coincidentally matched.
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

    Ok((board_id, ids[2], ids[0], ids[1])) // board, earliest, middle, latest
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_uses_board_task_sort_field_by_default() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let (board_id, earliest, middle, latest) = seed_three_cards_with_due_dates(&mut ctx).await?;

    ctx.execute(vec![Command::Board(BoardCommand::SetTaskSort(
        SetBoardTaskSort {
            board_id,
            field: SortField::DueDate,
            order: SortOrder::Ascending,
        },
    ))])?;

    let summaries = ctx.list_cards(CardListFilter {
        board_id: Some(board_id),
        ..Default::default()
    })?;

    let ids: Vec<Uuid> = summaries.iter().map(|s| s.id).collect();
    assert_eq!(
        ids,
        vec![earliest, middle, latest],
        "list_cards must sort by board.task_sort_field when no override is given"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_with_explicit_sort_overrides_board_default() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let (board_id, earliest, middle, latest) = seed_three_cards_with_due_dates(&mut ctx).await?;

    // Board default would sort by CardNumber (the default), but caller
    // explicitly asks for DueDate ascending.
    ctx.update_board(
        board_id,
        BoardUpdate {
            task_sort_field: Some(SortField::Default),
            task_sort_order: Some(SortOrder::Ascending),
            ..Default::default()
        },
    )?;

    let summaries = ctx.list_cards(CardListFilter {
        board_id: Some(board_id),
        sort: Some(SortField::DueDate),
        sort_order: Some(SortOrder::Ascending),
        ..Default::default()
    })?;

    let ids: Vec<Uuid> = summaries.iter().map(|s| s.id).collect();
    assert_eq!(
        ids,
        vec![earliest, middle, latest],
        "explicit sort override must beat board default"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_descending_order_reverses_result() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let (board_id, earliest, middle, latest) = seed_three_cards_with_due_dates(&mut ctx).await?;

    let summaries = ctx.list_cards(CardListFilter {
        board_id: Some(board_id),
        sort: Some(SortField::DueDate),
        sort_order: Some(SortOrder::Descending),
        ..Default::default()
    })?;

    let ids: Vec<Uuid> = summaries.iter().map(|s| s.id).collect();
    assert_eq!(ids, vec![latest, middle, earliest]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_tiebreaker_is_card_number_for_equal_keys() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let (board_id, _, _, _) = seed_three_cards_with_due_dates(&mut ctx).await?;

    // All cards default to `priority = Medium`, so sorting by Priority
    // ties — every result must come out in card_number order.
    let summaries = ctx.list_cards(CardListFilter {
        board_id: Some(board_id),
        sort: Some(SortField::Priority),
        sort_order: Some(SortOrder::Ascending),
        ..Default::default()
    })?;

    let card_numbers: Vec<u32> = summaries.iter().map(|s| s.card_number).collect();
    let mut sorted = card_numbers.clone();
    sorted.sort();
    assert_eq!(
        card_numbers, sorted,
        "tied primary keys must order by ascending card_number"
    );
    Ok(())
}
