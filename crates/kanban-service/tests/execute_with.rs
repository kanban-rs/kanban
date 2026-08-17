//! `execute_with` builds its batch INSIDE the transaction.
//!
//! The point is the rollback boundary, not the builder. A card number minted
//! outside the transaction survives a failed batch, which is how a rejected
//! create came to reserve a number no card ever carried. Anything the builder
//! writes must roll back with the batch.

use kanban_core::AppConfig;
use kanban_domain::commands::{CardCommand, Command, CreateCard};
use kanban_domain::{CreateCardOptions, KanbanOperations, Prefix};
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

/// The seam itself: a builder write must not outlive a failed batch.
///
/// Uses the prefix row because that is the write the whole card exists for,
/// but the assertion is about the transaction boundary, not about prefixes.
#[tokio::test(flavor = "multi_thread")]
async fn test_execute_with_rolls_back_a_builder_write_when_a_command_fails() {
    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = c.create_column(board.id, "Todo".into(), None).unwrap();

    let result = c.execute_with(|store| {
        store.upsert_prefix(Prefix {
            name: "builder".into(),
            card_counter: 42,
            sprint_counter: 0,
        })?;
        // A card into a column that does not exist: the command fails, so the
        // batch rolls back and the write above must go with it.
        Ok(vec![Command::Card(CardCommand::Create(CreateCard {
            id: Uuid::new_v4(),
            card_number: 1,
            board_id: board.id,
            column_id: Uuid::new_v4(),
            title: "doomed".into(),
            position: 0,
            options: CreateCardOptions::default(),
            timestamp: chrono::Utc::now(),
            default_card_prefix: "task".to_string(),
        }))])
    });

    assert!(result.is_err(), "the batch must fail");
    assert!(
        c.backend().get_prefix("builder").unwrap().is_none(),
        "the builder's write must roll back with the batch it belongs to; \
         surviving it is exactly how an allocation outlives the create it was \
         minted for"
    );

    // The column is untouched, so a normal create still works afterwards.
    let ok = c
        .create_card(
            board.id,
            col.id,
            "fine".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    assert_eq!(ok.card_number, 1);
}

/// The batch the builder produced is what gets recorded, so undo has something
/// to reverse. A builder whose commands never escaped the closure would leave
/// an empty undo entry and the create would be unundoable.
#[tokio::test(flavor = "multi_thread")]
async fn test_execute_with_records_the_built_batch_for_undo() {
    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = c.create_column(board.id, "Todo".into(), None).unwrap();
    let id = Uuid::new_v4();

    c.execute_with(|_store| {
        Ok(vec![Command::Card(CardCommand::Create(CreateCard {
            id,
            card_number: 7,
            board_id: board.id,
            column_id: col.id,
            title: "built".into(),
            position: 0,
            options: CreateCardOptions::default(),
            timestamp: chrono::Utc::now(),
            default_card_prefix: "task".to_string(),
        }))])
    })
    .unwrap();

    assert!(c.get_card(id).unwrap().is_some(), "the card was created");
    assert!(c.undo().unwrap(), "the built batch must be undoable");
    assert!(
        c.get_card(id).unwrap().is_none(),
        "undo must reverse the commands the BUILDER produced, not an empty batch"
    );
}

/// A builder that fails must abort the whole thing, leaving no partial batch
/// and nothing on the undo stack.
#[tokio::test(flavor = "multi_thread")]
async fn test_execute_with_propagates_a_builder_error_and_records_nothing() {
    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;
    let board = c.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = c.create_column(board.id, "Todo".into(), None).unwrap();

    let before = c
        .create_card(board.id, col.id, "one".into(), CreateCardOptions::default())
        .unwrap();

    let result =
        c.execute_with(|_store| Err(kanban_domain::KanbanError::validation("builder said no")));
    assert!(result.is_err(), "a builder error must surface");

    // Undo now reverses the earlier create, proving the failed builder pushed
    // nothing onto the stack.
    assert!(c.undo().unwrap());
    assert!(
        c.get_card(before.id).unwrap().is_none(),
        "the undo stack must be untouched by a failed builder"
    );
}
