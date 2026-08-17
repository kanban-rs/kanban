//! `CreateSubcardCommand::execute` must respect the target column's WIP
//! limit, the same as every other path that puts a card in a column.

use kanban_core::AppConfig;
use kanban_domain::commands::{Command, CreateSubcardCommand, DependencyCommand};
use kanban_domain::{ColumnUpdate, FieldUpdate, KanbanOperations};
use kanban_service::KanbanContext;
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;

async fn ctx(path: &std::path::Path) -> KanbanContext {
    let backend = kanban_persistence_sqlite::SqliteBackend::open(path.to_str().unwrap())
        .await
        .expect("open sqlite backend");
    KanbanContext::open(Arc::new(backend), AppConfig::default())
        .await
        .expect("open context")
}

fn counter(ctx: &KanbanContext, name: &str) -> u32 {
    ctx.backend()
        .get_prefix(name)
        .unwrap()
        .map_or(0, |p| p.card_counter)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_subcard_into_a_full_column_is_rejected() {
    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = c.create_column(board.id, "Todo".into(), None).unwrap();
    c.update_column(
        col.id,
        ColumnUpdate {
            wip_limit: FieldUpdate::Set(1),
            ..Default::default()
        },
    )
    .unwrap();
    let parent = c
        .create_card(board.id, col.id, "Parent".into(), Default::default())
        .unwrap();

    let subcard_id = Uuid::new_v4();
    let result = c.execute(vec![Command::Dependency(DependencyCommand::CreateSubcard(
        CreateSubcardCommand {
            id: subcard_id,
            parent_id: parent.id,
            board_id: board.id,
            column_id: col.id,
            title: "Subcard".into(),
            description: None,
            position: 1,
        },
    ))]);

    assert!(
        result.is_err(),
        "the column is already at its WIP limit of 1, so the subcard must be rejected"
    );
    assert_eq!(
        c.backend().list_cards_by_column(col.id).unwrap().len(),
        1,
        "the rejected subcard must not have been written to the column"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_subcard_into_a_column_with_room_still_succeeds() {
    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = c.create_column(board.id, "Todo".into(), None).unwrap();
    c.update_column(
        col.id,
        ColumnUpdate {
            wip_limit: FieldUpdate::Set(2),
            ..Default::default()
        },
    )
    .unwrap();
    let parent = c
        .create_card(board.id, col.id, "Parent".into(), Default::default())
        .unwrap();

    let subcard_id = Uuid::new_v4();
    c.execute(vec![Command::Dependency(DependencyCommand::CreateSubcard(
        CreateSubcardCommand {
            id: subcard_id,
            parent_id: parent.id,
            board_id: board.id,
            column_id: col.id,
            title: "Subcard".into(),
            description: None,
            position: 1,
        },
    ))])
    .expect("column has room, so the subcard must be created");

    assert_eq!(c.backend().list_cards_by_column(col.id).unwrap().len(), 2);
    assert!(
        c.graph().unwrap().contains(parent.id, subcard_id),
        "the parent edge must still be set"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_rejected_subcard_does_not_consume_a_card_number() {
    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = c.create_column(board.id, "Todo".into(), None).unwrap();
    c.update_column(
        col.id,
        ColumnUpdate {
            wip_limit: FieldUpdate::Set(1),
            ..Default::default()
        },
    )
    .unwrap();
    let parent = c
        .create_card(board.id, col.id, "Parent".into(), Default::default())
        .unwrap();

    let before = counter(&c, "kan");

    let subcard_id = Uuid::new_v4();
    let result = c.execute(vec![Command::Dependency(DependencyCommand::CreateSubcard(
        CreateSubcardCommand {
            id: subcard_id,
            parent_id: parent.id,
            board_id: board.id,
            column_id: col.id,
            title: "Subcard".into(),
            description: None,
            position: 1,
        },
    ))]);

    assert!(result.is_err(), "the column is full, so this must fail");
    assert_eq!(
        counter(&c, "kan"),
        before,
        "the rejected subcard's allocation must be rolled back with the batch"
    );
}

/// The WIP check is `CreateSubcardCommand`'s ONLY column validation — it has
/// no `require_column` of its own — so it is what stops a subcard being
/// created into a column that does not exist.
///
/// That hole was open before the check was added, which is why the pre-existing
/// `test_create_subcard_command` passed while addressing a bare `Uuid` that was
/// never inserted as a column. Pinned here because it is now load-bearing:
/// making the WIP check conditional, or moving it below the writes, silently
/// reopens the hole.
#[tokio::test(flavor = "multi_thread")]
async fn test_create_subcard_into_a_nonexistent_column_is_rejected() {
    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = c.create_column(board.id, "Todo".into(), None).unwrap();
    let parent = c
        .create_card(board.id, col.id, "Parent".into(), Default::default())
        .unwrap();

    let subcard_id = Uuid::new_v4();
    let result = c.execute(vec![Command::Dependency(DependencyCommand::CreateSubcard(
        CreateSubcardCommand {
            id: subcard_id,
            parent_id: parent.id,
            board_id: board.id,
            column_id: Uuid::new_v4(),
            title: "Orphan".into(),
            description: None,
            position: 0,
        },
    ))]);

    assert!(
        result.is_err(),
        "a subcard must not be created into a column that does not exist"
    );
    assert!(
        c.get_card(subcard_id).unwrap().is_none(),
        "and no card may be left behind"
    );
}
