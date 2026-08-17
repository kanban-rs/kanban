//! `default_card_prefix` must reach the card allocator.
//!
//! A board with no prefix of its own falls through to the workspace default.
//! Resolving that from the compile-time constant instead of the configured
//! value makes the setting inert on the card axis while it works on the sprint
//! axis, and files every card under a namespace the user did not choose.

use kanban_domain::commands::{Command, CreateSubcardCommand, DependencyCommand};
use kanban_domain::{BoardUpdate, CreateCardOptions, FieldUpdate, KanbanOperations};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;
use tempfile::tempdir;
use uuid::Uuid;

async fn open_with(path: &std::path::Path, default_card_prefix: Option<&str>) -> KanbanContext {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
    let config = AppConfig {
        default_card_prefix: default_card_prefix.map(Into::into),
        ..Default::default()
    };
    KanbanContext::open(backend, config).await.unwrap()
}

fn card_counter_of(ctx: &KanbanContext, name: &str) -> u32 {
    ctx.backend()
        .get_prefix(name)
        .unwrap()
        .map_or(0, |p| p.card_counter)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_card_creation_allocates_from_the_configured_default_card_prefix() {
    let dir = tempdir().unwrap();
    let mut ctx = open_with(&dir.path().join("s.json"), Some("feat")).await;

    let board = ctx.create_board("B".into(), None).unwrap();
    let column = ctx.create_column(board.id, "TODO".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "one".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    assert_eq!(
        card.prefix, "feat",
        "the card was stamped with the wrong namespace"
    );
    assert_eq!(card_counter_of(&ctx, "feat"), 1);
    assert_eq!(
        card_counter_of(&ctx, "task"),
        0,
        "a namespace the user did not configure was allocated from"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_an_unset_configured_default_still_allocates_from_the_compile_time_default() {
    let dir = tempdir().unwrap();
    let mut ctx = open_with(&dir.path().join("s.json"), None).await;

    let board = ctx.create_board("B".into(), None).unwrap();
    let column = ctx.create_column(board.id, "TODO".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "one".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    assert_eq!(card.prefix, "task");
    assert_eq!(card_counter_of(&ctx, "task"), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_boards_own_prefix_still_wins_over_the_configured_default() {
    let dir = tempdir().unwrap();
    let mut ctx = open_with(&dir.path().join("s.json"), Some("feat")).await;

    let board = ctx.create_board("B".into(), None).unwrap();
    ctx.update_board(
        board.id,
        BoardUpdate {
            card_prefix: FieldUpdate::Set("KAN".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let column = ctx.create_column(board.id, "TODO".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "one".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    assert_eq!(card.prefix, "KAN", "prefix casing is stored as configured");
    assert_eq!(card_counter_of(&ctx, "feat"), 0);
}

/// Subcards draw from the same counter as their siblings, so they must resolve
/// the default the same way. Drawing from a different namespace hands a subcard
/// a number one of its siblings already holds.
///
/// The command is dispatched directly because nothing in the product builds one
/// yet, so this pins the command's own resolution rather than a caller's.
#[tokio::test(flavor = "multi_thread")]
async fn test_subcard_creation_allocates_from_the_configured_default_card_prefix() {
    let dir = tempdir().unwrap();
    let mut ctx = open_with(&dir.path().join("s.json"), Some("feat")).await;

    let board = ctx.create_board("B".into(), None).unwrap();
    let column = ctx.create_column(board.id, "TODO".into(), None).unwrap();
    let parent = ctx
        .create_card(
            board.id,
            column.id,
            "parent".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    let subcard_id = Uuid::new_v4();
    ctx.execute(vec![Command::Dependency(DependencyCommand::CreateSubcard(
        CreateSubcardCommand {
            id: subcard_id,
            parent_id: parent.id,
            board_id: board.id,
            column_id: column.id,
            title: "child".into(),
            description: None,
            position: 1,
            default_card_prefix: "feat".into(),
        },
    ))])
    .unwrap();

    let subcard = ctx.get_card(subcard_id).unwrap().unwrap();
    assert_eq!(subcard.prefix, "feat");
    assert_eq!(
        subcard.card_number, 2,
        "the subcard drew from a different counter than its parent"
    );
    assert_eq!(card_counter_of(&ctx, "feat"), 2);
    assert_eq!(card_counter_of(&ctx, "task"), 0);
}
