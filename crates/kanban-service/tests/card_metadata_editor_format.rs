//! End-to-end pipeline coverage for the card-metadata editor format,
//! pinning the YYYY-MM-DD / RFC3339 contract from the JSON wire format
//! all the way through to the persisted card.

use chrono::TimeZone;
use kanban_backend_memory::InMemoryStore;
use kanban_core::AppConfig;
use kanban_domain::commands::{ApplyCardMetadata, CardCommand, Command};
use kanban_domain::editable::CardMetadataDto;
use kanban_domain::{KanbanOperations, KanbanResult};
use kanban_service::KanbanContext;
use std::sync::Arc;

async fn make_ctx() -> KanbanContext {
    KanbanContext::open(Arc::new(InMemoryStore::new()), AppConfig::default())
        .await
        .unwrap()
}

fn metadata_json(due_date_value: &str) -> String {
    format!(r#"{{"priority":"High","status":"Todo","points":null,"due_date":{due_date_value}}}"#)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_editor_yyyy_mm_dd_due_date_is_persisted_as_midnight_utc() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "C".into(), Default::default())?;

    let dto: CardMetadataDto = serde_json::from_str(&metadata_json(r#""2024-01-15""#))
        .expect("YYYY-MM-DD must deserialize through the editor DTO");

    ctx.execute(vec![Command::Card(CardCommand::ApplyMetadata(
        ApplyCardMetadata {
            card_id: card.id,
            dto,
        },
    ))])?;

    let stored = ctx.get_card(card.id)?.unwrap();
    assert_eq!(
        stored.due_date,
        Some(chrono::Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap()),
        "YYYY-MM-DD must persist as midnight UTC on the named day"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_editor_rfc3339_due_date_is_persisted_at_exact_instant() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "C".into(), Default::default())?;

    let dto: CardMetadataDto =
        serde_json::from_str(&metadata_json(r#""2024-01-15T14:30:00Z""#)).unwrap();

    ctx.execute(vec![Command::Card(CardCommand::ApplyMetadata(
        ApplyCardMetadata {
            card_id: card.id,
            dto,
        },
    ))])?;

    let stored = ctx.get_card(card.id)?.unwrap();
    assert_eq!(
        stored.due_date,
        Some(
            chrono::Utc
                .with_ymd_and_hms(2024, 1, 15, 14, 30, 0)
                .unwrap()
        ),
        "RFC3339 must persist at the exact instant the user supplied"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_editor_garbage_due_date_fails_at_json_parse_before_command_execution(
) -> KanbanResult<()> {
    let result = serde_json::from_str::<CardMetadataDto>(&metadata_json(r#""yesterday""#));
    let err = result.expect_err("garbage date must fail JSON deserialization");
    let msg = err.to_string();
    assert!(
        msg.contains("yesterday") && msg.contains("YYYY-MM-DD"),
        "the serde error must mention the input and the supported format \
         so the TUI banner can show actionable feedback; got: {msg}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_editor_yyyy_mm_dd_due_date_round_trips_back_through_serializer() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "C".into(), Default::default())?;

    let dto: CardMetadataDto = serde_json::from_str(&metadata_json(r#""2024-01-15""#)).unwrap();
    ctx.execute(vec![Command::Card(CardCommand::ApplyMetadata(
        ApplyCardMetadata {
            card_id: card.id,
            dto,
        },
    ))])?;

    let stored = ctx.get_card(card.id)?.unwrap();
    let re_dto = CardMetadataDto {
        priority: format!("{:?}", stored.priority),
        status: format!("{:?}", stored.status),
        points: stored.points,
        due_date: stored.due_date,
    };
    let json = serde_json::to_string(&re_dto).unwrap();
    assert!(
        json.contains(r#""due_date":"2024-01-15""#),
        "what the user enters as YYYY-MM-DD should come back as YYYY-MM-DD on re-open; got: {json}"
    );
    Ok(())
}
