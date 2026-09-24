//! `KanbanContext::issued_by` stamps every recorded `CommandBatch` with the
//! identity of the client that requested the mutation.

use kanban_backend_memory::InMemoryStore;
use kanban_core::{AppConfig, ClientId};
use kanban_domain::{KanbanOperations, KanbanResult};
use kanban_service::KanbanContext;
use std::sync::Arc;

async fn make_ctx() -> KanbanContext {
    KanbanContext::open(Arc::new(InMemoryStore::new()), AppConfig::default())
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_open_deferred_defaults_issued_by_to_nil() {
    let ctx = KanbanContext::open_deferred(Arc::new(InMemoryStore::new()), AppConfig::default());
    assert_eq!(ctx.issued_by(), ClientId::nil());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_execute_stamps_context_issued_by_on_batch() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let backend = ctx.backend();
    let baseline = backend.batch_count()?;

    let id = ClientId::new();
    ctx.set_issued_by(id);
    let _ = ctx.create_board_impl("B".into(), None)?;

    let batches = backend.load_batches(baseline, baseline + 1)?;
    assert_eq!(batches[0].issued_by, id);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_board_stamps_context_issued_by_on_batch() -> KanbanResult<()> {
    let mut a = make_ctx().await;
    let board = a.create_board("Source".to_string(), Some("SRC".to_string()))?;
    a.create_column(board.id, "To Do".to_string(), None)?;
    let json = a.export_board(Some(board.id))?;

    let mut b = make_ctx().await;
    let backend = b.backend();
    let baseline = backend.batch_count()?;

    let id = ClientId::new();
    b.set_issued_by(id);
    let _ = b.import_board_impl(&json)?;

    let batches = backend.load_batches(baseline, baseline + 1)?;
    assert_eq!(batches[0].issued_by, id);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_set_issued_by_applies_to_subsequent_batches_only() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let backend = ctx.backend();
    let baseline = backend.batch_count()?;

    let _ = ctx.create_board_impl("Before".into(), None)?;

    let id = ClientId::new();
    ctx.set_issued_by(id);
    let _ = ctx.create_board_impl("After".into(), None)?;

    let batches = backend.load_batches(baseline, baseline + 2)?;
    assert_eq!(batches[0].issued_by, ClientId::nil());
    assert_eq!(batches[1].issued_by, id);
    Ok(())
}
