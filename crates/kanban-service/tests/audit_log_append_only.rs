//! Audit log invariants: every forward `execute()` appends one entry,
//! and no `KanbanContext` path removes one. The UndoStack rewinds and
//! truncates its redo tail independently; the audit log does not.

use kanban_backend_memory::InMemoryStore;
use kanban_core::AppConfig;
use kanban_domain::commands::{BoardCommand, Command, CreateBoard};
use kanban_domain::KanbanResult;
use kanban_service::KanbanContext;
use std::sync::Arc;
use uuid::Uuid;

async fn make_ctx() -> KanbanContext {
    KanbanContext::open(Arc::new(InMemoryStore::new()), AppConfig::default())
        .await
        .unwrap()
}

fn create_board_cmd(name: &str, position: i32) -> (Uuid, Command) {
    let id = Uuid::new_v4();
    (
        id,
        Command::Board(BoardCommand::Create(CreateBoard {
            id,
            name: name.into(),
            card_prefix: None,
            position,
        })),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn test_audit_log_records_all_forward_executes_in_order() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let backend = ctx.backend();
    let baseline = backend.batch_count()?;

    let (_, cmd_a) = create_board_cmd("A", 0);
    let (_, cmd_b) = create_board_cmd("B", 1);
    let (_, cmd_c) = create_board_cmd("C", 2);

    ctx.execute(vec![cmd_a])?;
    assert_eq!(backend.batch_count()?, baseline + 1);
    ctx.execute(vec![cmd_b])?;
    assert_eq!(backend.batch_count()?, baseline + 2);
    ctx.execute(vec![cmd_c])?;
    assert_eq!(backend.batch_count()?, baseline + 3);

    let batches = backend.load_batches(baseline, baseline + 3)?;
    assert_eq!(batches.len(), 3);
    let names: Vec<String> = batches
        .iter()
        .filter_map(|batch| match batch.commands.first() {
            Some(Command::Board(BoardCommand::Create(c))) => Some(c.name.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(names, vec!["A", "B", "C"]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_audit_log_does_not_rewind_on_undo() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let backend = ctx.backend();
    let baseline = backend.batch_count()?;

    let (_, cmd_a) = create_board_cmd("A", 0);
    let (_, cmd_b) = create_board_cmd("B", 1);

    ctx.execute(vec![cmd_a])?;
    ctx.execute(vec![cmd_b])?;
    assert_eq!(backend.batch_count()?, baseline + 2);

    // Undo B. UndoStack cursor moves back; audit log is untouched —
    // "the user undid B" is itself an event, not a deletion.
    assert!(ctx.undo()?);
    assert_eq!(
        backend.batch_count()?,
        baseline + 2,
        "undo must not rewind the audit log"
    );

    // Undo A. Same story.
    assert!(ctx.undo()?);
    assert_eq!(
        backend.batch_count()?,
        baseline + 2,
        "undo must not rewind the audit log even when cursor reaches zero"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_audit_log_preserves_abandoned_redo_tail_on_branching_execute() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let backend = ctx.backend();
    let baseline = backend.batch_count()?;

    let (_, cmd_a) = create_board_cmd("A", 0);
    let (_, cmd_b) = create_board_cmd("B", 1);
    let (_, cmd_c) = create_board_cmd("C", 2);

    ctx.execute(vec![cmd_a])?;
    ctx.execute(vec![cmd_b])?;

    // Undo B — B is now in the UndoStack's redo tail.
    assert!(ctx.undo()?);
    assert_eq!(ctx.redo_depth(), 1, "B is parked in the redo tail");

    // Execute C — UndoStack drops B from its tail (the user branched
    // off the partial undo). The audit log, however, must still
    // record that B happened.
    ctx.execute(vec![cmd_c])?;
    assert_eq!(
        ctx.redo_depth(),
        0,
        "executing past an undo truncates the redo tail"
    );

    assert_eq!(
        backend.batch_count()?,
        baseline + 3,
        "audit log must record A, B, AND C — B is gone from the undo \
         stack but the fact that it happened cannot be unhappened"
    );

    let batches = backend.load_batches(baseline, baseline + 3)?;
    let names: Vec<String> = batches
        .iter()
        .filter_map(|batch| match batch.commands.first() {
            Some(Command::Board(BoardCommand::Create(c))) => Some(c.name.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        names,
        vec!["A", "B", "C"],
        "audit log order is the order of forward execution, including \
         entries the user later branched away from"
    );
    Ok(())
}
