//! C10-Foundation (KAN-862): an archived board is a READ-ONLY SHELF. Content
//! mutations targeting an archived board (or an entity belonging to one) are
//! rejected with `BoardArchived`; lifecycle ops (restore, permanent-delete) and
//! reads stay allowed. Enforced once at the service `execute` seam so every
//! surface inherits identical behavior.

use kanban_domain::{
    BoardUpdate, CardListFilter, ColumnUpdate, InMemoryStore, KanbanOperations, KanbanResult,
};
use kanban_service::{AppConfig, KanbanContext};
use std::sync::Arc;
use uuid::Uuid;

async fn ctx() -> KanbanContext {
    KanbanContext::open(Arc::new(InMemoryStore::new()), AppConfig::default())
        .await
        .unwrap()
}

/// Board with two columns and one card, then archived.
/// Returns (board_id, col_a, col_b, card_id).
fn seed_archived(c: &mut KanbanContext) -> KanbanResult<(Uuid, Uuid, Uuid, Uuid)> {
    let b = c.create_board("Proj".into(), None)?;
    let col_a = c.create_column(b.id, "Todo".into(), None)?;
    let col_b = c.create_column(b.id, "Doing".into(), None)?;
    let card = c.create_card(b.id, col_a.id, "task".into(), Default::default())?;
    c.archive_board(b.id)?;
    Ok((b.id, col_a.id, col_b.id, card.id))
}

// ── Rejected: content mutations on an archived board ───────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn test_create_card_on_archived_board_is_rejected() -> KanbanResult<()> {
    let mut c = ctx().await;
    let (board, col_a, _col_b, _card) = seed_archived(&mut c)?;
    let err = c
        .create_card(board, col_a, "new".into(), Default::default())
        .expect_err("create_card on an archived board must be rejected");
    assert!(err.is_board_archived(), "got: {err}");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_move_card_within_archived_board_is_rejected() -> KanbanResult<()> {
    let mut c = ctx().await;
    let (_board, _col_a, col_b, card) = seed_archived(&mut c)?;
    let err = c
        .move_card(card, col_b, None)
        .expect_err("moving a card in an archived board must be rejected");
    assert!(err.is_board_archived(), "got: {err}");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_column_on_archived_board_is_rejected() -> KanbanResult<()> {
    let mut c = ctx().await;
    let (_board, col_a, _col_b, _card) = seed_archived(&mut c)?;
    let err = c
        .update_column(
            col_a,
            ColumnUpdate {
                name: Some("Renamed".into()),
                ..Default::default()
            },
        )
        .expect_err("editing an archived board's column must be rejected");
    assert!(err.is_board_archived(), "got: {err}");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_board_settings_on_archived_board_is_rejected() -> KanbanResult<()> {
    let mut c = ctx().await;
    let (board, _col_a, _col_b, _card) = seed_archived(&mut c)?;
    let err = c
        .update_board(
            board,
            BoardUpdate {
                name: Some("Renamed".into()),
                ..Default::default()
            },
        )
        .expect_err("editing an archived board's settings must be rejected");
    assert!(err.is_board_archived(), "got: {err}");
    Ok(())
}

// ── Allowed: lifecycle ops + live boards + post-restore ────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn test_restore_archived_board_is_allowed() -> KanbanResult<()> {
    let mut c = ctx().await;
    let (board, _col_a, _col_b, _card) = seed_archived(&mut c)?;
    c.restore_board(board)?; // lifecycle op must NOT be gated
    assert!(c.get_board(board)?.is_some(), "board is live again");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_permanent_delete_of_archived_board_is_allowed() -> KanbanResult<()> {
    let mut c = ctx().await;
    let (board, _col_a, _col_b, _card) = seed_archived(&mut c)?;
    c.delete_board(board)?; // permanent delete of an archived board is a lifecycle op
    assert!(c.get_board(board)?.is_none());
    assert!(c
        .list_archived_boards()?
        .iter()
        .all(|ab| ab.entity.id != board));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_card_on_live_board_still_works() -> KanbanResult<()> {
    let mut c = ctx().await;
    let b = c.create_board("Live".into(), None)?;
    let col = c.create_column(b.id, "Todo".into(), None)?;
    let card = c.create_card(b.id, col.id, "task".into(), Default::default())?;
    assert!(c.get_card(card.id)?.is_some());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_sprint_on_archived_board_is_rejected() -> KanbanResult<()> {
    let mut c = ctx().await;
    let (board, _col_a, _col_b, _card) = seed_archived(&mut c)?;
    let err = c
        .create_sprint(board, None, None)
        .expect_err("creating a sprint on an archived board must be rejected");
    assert!(err.is_board_archived(), "got: {err}");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_mutation_after_restore_succeeds() -> KanbanResult<()> {
    let mut c = ctx().await;
    let (board, col_a, _col_b, _card) = seed_archived(&mut c)?;
    c.restore_board(board)?;
    // Now editable again.
    let card = c.create_card(board, col_a, "new".into(), Default::default())?;
    let listed = c.list_cards(CardListFilter {
        board_id: Some(board),
        ..Default::default()
    })?;
    assert!(listed.iter().any(|s| s.id == card.id));
    Ok(())
}

// ── Through the real SQLite backend ────────────────────────────────────────
// All-backend coverage (CLAUDE.md): the guard reads via the backend, and the
// "editable after restore" case only reaches SQLite because KAN-863 fixed
// restore to preserve the subtree.
mod sqlite_gate {
    use super::*;
    use kanban_service::open_context;
    use tempfile::TempDir;

    async fn open_sqlite(path: &std::path::Path) -> KanbanContext {
        open_context(path.to_str().unwrap(), AppConfig::default())
            .await
            .unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_write_gate_through_sqlite() -> KanbanResult<()> {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("gate.sqlite3");
        let mut c = open_sqlite(&path).await;
        let b = c.create_board("Proj".into(), None)?;
        let col = c.create_column(b.id, "Todo".into(), None)?;
        c.create_card(b.id, col.id, "task".into(), Default::default())?;
        c.archive_board(b.id)?;

        // Content mutations rejected with BoardArchived through SQLite.
        assert!(c
            .create_card(b.id, col.id, "new".into(), Default::default())
            .expect_err("create rejected")
            .is_board_archived());
        assert!(c
            .update_column(
                col.id,
                ColumnUpdate {
                    name: Some("x".into()),
                    ..Default::default()
                }
            )
            .expect_err("update rejected")
            .is_board_archived());

        // Lifecycle allowed, then editable again after restore.
        c.restore_board(b.id)?;
        c.create_card(b.id, col.id, "after".into(), Default::default())?;
        let listed = c.list_cards(CardListFilter {
            board_id: Some(b.id),
            ..Default::default()
        })?;
        assert_eq!(
            listed.len(),
            2,
            "subtree preserved + editable after restore"
        );
        Ok(())
    }
}
