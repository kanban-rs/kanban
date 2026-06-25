//! Service-tier integration tests for `KanbanContext::create_card_from_spec`
//! (Card factory epic slice, KAN-795): the rich create funnel that derives the
//! owning board from `column.board_id`, validates the column FK (required) and
//! sprint FK (optional + board match), resolves the optional client id with a
//! uniqueness check across live AND archived cards, mints `card_number` from the
//! board counter (bumping it), and dispatches through `Card::create` via the
//! frozen `CreateCard` command.
use kanban_domain::{
    CardPriority, CardStatus, CreateCardOptions, KanbanError, KanbanOperations, NewCard,
};
use kanban_service::{open_context, AppConfig, KanbanContext};
use tempfile::TempDir;
use uuid::Uuid;

async fn ctx() -> (TempDir, KanbanContext) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.kanban").to_string_lossy().to_string();
    let ctx = open_context(&path, AppConfig::default()).await.unwrap();
    (dir, ctx)
}

fn spec(column_id: Uuid, title: &str) -> NewCard {
    NewCard {
        column_id,
        title: title.to_string(),
        description: None,
        priority: CardPriority::Medium,
        due_date: None,
        points: None,
        sprint_id: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_card_funnels_through_factory_seeds_defaults() {
    let (_d, mut ctx) = ctx().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();

    let card = ctx
        .create_card_from_spec(None, spec(col.id, "First"))
        .unwrap();

    assert_eq!(card.status, CardStatus::Todo);
    assert_eq!(card.completed_at, None);
    assert_eq!(card.position, 0, "first card appended at position 0");
    // card_number minted from board.card_counter (seeded at 1), board bumped.
    assert_eq!(card.card_number, 1);
    let bumped = ctx.get_board(board.id).unwrap().unwrap();
    assert_eq!(bumped.card_counter, 2, "board counter incremented by 1");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_card_mints_id_when_not_supplied() {
    let (_d, mut ctx) = ctx().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();

    let a = ctx.create_card_from_spec(None, spec(col.id, "A")).unwrap();
    let b = ctx.create_card_from_spec(None, spec(col.id, "B")).unwrap();

    assert_ne!(a.id, Uuid::nil());
    assert_ne!(a.id, b.id, "two minted ids differ");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_card_uses_client_supplied_id() {
    let (_d, mut ctx) = ctx().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();

    let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let card = ctx
        .create_card_from_spec(Some(id), spec(col.id, "Pinned"))
        .unwrap();

    assert_eq!(card.id, id);
    assert_eq!(ctx.get_card(id).unwrap().unwrap().id, id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_card_with_duplicate_client_id_returns_conflict() {
    let (_d, mut ctx) = ctx().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();

    let id = Uuid::new_v4();
    ctx.create_card_from_spec(Some(id), spec(col.id, "Original"))
        .unwrap();

    let err = ctx
        .create_card_from_spec(Some(id), spec(col.id, "Collision"))
        .unwrap_err();
    assert!(
        err.is_already_exists(),
        "expected AlreadyExists conflict, got: {err:?}"
    );

    let existing = ctx.get_card(id).unwrap().unwrap();
    assert_eq!(
        existing.title, "Original",
        "collision rejected before write"
    );
    assert_eq!(ctx.list_all_cards().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_card_with_duplicate_archived_id_returns_conflict() {
    let (_d, mut ctx) = ctx().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();

    let id = Uuid::new_v4();
    ctx.create_card_from_spec(Some(id), spec(col.id, "ToArchive"))
        .unwrap();
    ctx.archive_card(id).unwrap();

    let err = ctx
        .create_card_from_spec(Some(id), spec(col.id, "Collision"))
        .unwrap_err();
    assert!(
        err.is_already_exists(),
        "expected AlreadyExists conflict against an archived id, got: {err:?}"
    );
    assert!(ctx.get_card(id).unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_card_with_missing_column_returns_not_found() {
    let (_d, mut ctx) = ctx().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    // Real column so the board counter is observable; we create against a bogus one.
    ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let before = ctx.get_board(board.id).unwrap().unwrap().card_counter;

    let bogus = Uuid::new_v4();
    let err = ctx
        .create_card_from_spec(None, spec(bogus, "Orphan"))
        .unwrap_err();

    assert!(err.is_not_found(), "expected NotFound, got: {err:?}");
    assert_eq!(
        ctx.get_board(board.id).unwrap().unwrap().card_counter,
        before,
        "board counter unchanged on a rejected create"
    );
    assert_eq!(ctx.list_all_cards().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_card_with_sprint_from_other_board_returns_mismatch() {
    let (_d, mut ctx) = ctx().await;
    let board_a = ctx.create_board("A".into(), None).unwrap();
    let board_b = ctx.create_board("B".into(), None).unwrap();
    let col_a = ctx.create_column(board_a.id, "Todo".into(), None).unwrap();
    let sprint_b = ctx.create_sprint(board_b.id, None, None).unwrap();

    let mut s = spec(col_a.id, "Card");
    s.sprint_id = Some(sprint_b.id);
    let err = ctx.create_card_from_spec(None, s).unwrap_err();

    assert!(
        err.is_sprint_board_mismatch(),
        "expected SprintBoardMismatch, got: {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_card_with_sprint_seeds_sprint_log() {
    let (_d, mut ctx) = ctx().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let sprint = ctx.create_sprint(board.id, None, None).unwrap();

    let mut s = spec(col.id, "Card");
    s.sprint_id = Some(sprint.id);
    let card = ctx.create_card_from_spec(None, s).unwrap();

    assert_eq!(card.sprint_id, Some(sprint.id));
    assert_eq!(card.sprint_logs.len(), 1, "service-tier sprint-log seeding");
    assert_eq!(card.sprint_logs[0].sprint_id, sprint.id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_card_applies_all_create_options_in_one_call() {
    let (_d, mut ctx) = ctx().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let due = "2024-06-01T00:00:00Z".parse().unwrap();

    let card = ctx
        .create_card_from_spec(
            None,
            NewCard {
                column_id: col.id,
                title: "Rich".to_string(),
                description: Some("desc".to_string()),
                priority: CardPriority::Critical,
                due_date: Some(due),
                points: Some(8),
                sprint_id: None,
            },
        )
        .unwrap();

    assert_eq!(card.description, Some("desc".to_string()));
    assert_eq!(card.priority, CardPriority::Critical);
    assert_eq!(card.points, Some(8));
    assert_eq!(card.due_date, Some(due));
    // Single create: no observable intermediate update (no follow-up patch).
    assert_eq!(
        card.updated_at, card.created_at,
        "create applies all fields in one Card::create call"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_create_card_is_idempotent_create_or_replace() {
    let (_d, mut ctx) = ctx().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();

    let id = Uuid::new_v4();
    let first = ctx
        .create_or_replace_card(id, spec(col.id, "Initial"))
        .unwrap();
    assert!(first.created, "absent id reports created");
    assert_eq!(first.card.id, id);
    assert_eq!(first.card.title, "Initial");

    let replacement = NewCard {
        column_id: col.id,
        title: "Replaced".to_string(),
        description: Some("new".to_string()),
        priority: CardPriority::High,
        due_date: None,
        points: Some(3),
        sprint_id: None,
    };
    let second = ctx.create_or_replace_card(id, replacement).unwrap();
    assert!(!second.created, "present id reports replace");
    assert_eq!(second.card.id, id, "id stable across replace");

    let fetched = ctx.get_card(id).unwrap().unwrap();
    assert_eq!(fetched.title, "Replaced");
    assert_eq!(fetched.description, Some("new".to_string()));
    assert_eq!(fetched.priority, CardPriority::High);
    assert_eq!(fetched.points, Some(3));
    assert_eq!(
        ctx.list_all_cards().unwrap().len(),
        1,
        "no duplicate created"
    );
}

/// The legacy `CreateCardOptions` shim path still works (no churn for the many
/// trait callers) and routes through the same factory funnel.
#[tokio::test(flavor = "multi_thread")]
async fn test_create_card_shim_delegates_to_spec_path() {
    let (_d, mut ctx) = ctx().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();

    let card = ctx
        .create_card(
            board.id,
            col.id,
            "Shimmed".into(),
            CreateCardOptions {
                description: Some("via shim".to_string()),
                priority: Some(CardPriority::Low),
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(card.title, "Shimmed");
    assert_eq!(card.description, Some("via shim".to_string()));
    assert_eq!(card.priority, CardPriority::Low);
    assert_eq!(card.status, CardStatus::Todo);
    let _ = KanbanError::not_found("x", Uuid::nil());
}
