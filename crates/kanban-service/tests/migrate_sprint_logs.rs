//! Integration tests for `KanbanContext::migrate_sprint_logs` (KAN-430).
//!
//! Run against both `JsonDataStore` and `SqliteBackend` via a macro to catch any
//! backend-specific divergence. The pure migration logic itself is unit-tested
//! in `kanban_domain::card_lifecycle::tests`.

use kanban_backend_memory::InMemoryStore;
use kanban_domain::{Board, Card, Column, KanbanError, KanbanOperations, Sprint, UndoOperations};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_persistence_sqlite::SqliteBackend;
use kanban_service::test_helpers::FaultInjectingBackend;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;
use tempfile::tempdir;

async fn open_json_ctx() -> (KanbanContext, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.json");
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(&path))));
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    (ctx, dir)
}

async fn open_sqlite_ctx() -> (KanbanContext, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.sqlite");
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(SqliteBackend::open(path.to_str().unwrap()).await.unwrap());
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    (ctx, dir)
}

macro_rules! migrate_sprint_logs_tests {
    ($mod_name:ident, $open_ctx:expr) => {
        mod $mod_name {
            use super::*;

            #[tokio::test(flavor = "multi_thread")]
            async fn test_migrate_sprint_logs_backfills_card_with_sprint_id_and_empty_logs() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let board = Board::new("B", Some("TST"));
                let col = Column::new(board.id, "Col", 0);
                let sprint = Sprint::new(board.id, 1, None, Some("Alpha"));
                let sprint_id = sprint.id;
                let mut card = Card::new(board.id, col.id, "Card", 0);
                let card_id = card.id;
                card.sprint_id = Some(sprint_id);
                assert!(card.sprint_logs.is_empty());
                backend.upsert_board(board).unwrap();
                backend.upsert_column(col).unwrap();
                backend.upsert_sprint(sprint).unwrap();
                backend.upsert_card(card).unwrap();

                let (migrated, _inv) = ctx.migrate_sprint_logs().unwrap();
                assert_eq!(migrated, 1);

                let card = backend.get_card(card_id).unwrap().unwrap();
                assert_eq!(
                    card.sprint_logs.len(),
                    1,
                    "sprint log should be backfilled for card with sprint_id but empty logs"
                );
                assert_eq!(card.sprint_logs[0].sprint_number, 1);
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_migrate_sprint_logs_no_op_when_nothing_to_migrate() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let board = Board::new("B", Some("TST"));
                let col = Column::new(board.id, "Col", 0);
                let card = Card::new(board.id, col.id, "Card", 0);
                backend.upsert_board(board).unwrap();
                backend.upsert_column(col).unwrap();
                backend.upsert_card(card).unwrap();

                let before = backend.list_all_cards().unwrap();
                let (migrated, _inv) = ctx.migrate_sprint_logs().unwrap();
                assert_eq!(
                    migrated, 0,
                    "migrate_sprint_logs should report zero when no card needs backfilling"
                );
                assert_eq!(
                    backend.list_all_cards().unwrap(),
                    before,
                    "no-op migration must not mutate any card"
                );
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_migrate_sprint_logs_only_backfills_eligible_cards_in_mixed_batch() {
                let (mut ctx, _dir) = $open_ctx.await;
                let backend = ctx.backend();

                let board = Board::new("B", Some("TST"));
                let col = Column::new(board.id, "Col", 0);
                let sprint = Sprint::new(board.id, 1, None, Some("Alpha"));
                let sprint_id = sprint.id;

                let mut card_needs_backfill = Card::new(board.id, col.id, "Needs Backfill", 0);
                card_needs_backfill.sprint_id = Some(sprint_id);
                let needs_backfill_id = card_needs_backfill.id;

                let mut card_already_logged = Card::new(board.id, col.id, "Already Logged", 1);
                card_already_logged.sprint_id = Some(sprint_id);
                card_already_logged
                    .sprint_logs
                    .push(kanban_domain::SprintLog::new(
                        sprint_id,
                        1,
                        None::<String>,
                        "Active",
                    ));
                let already_logged_id = card_already_logged.id;
                let already_logged_before = card_already_logged.sprint_logs.clone();

                let card_no_sprint = Card::new(board.id, col.id, "No Sprint", 2);
                let no_sprint_id = card_no_sprint.id;

                backend.upsert_board(board).unwrap();
                backend.upsert_column(col).unwrap();
                backend.upsert_sprint(sprint).unwrap();
                backend.upsert_card(card_needs_backfill).unwrap();
                backend.upsert_card(card_already_logged).unwrap();
                backend.upsert_card(card_no_sprint).unwrap();

                let (migrated, _inv) = ctx.migrate_sprint_logs().unwrap();
                assert_eq!(migrated, 1, "only the eligible card should be migrated");

                let backfilled = backend.get_card(needs_backfill_id).unwrap().unwrap();
                assert_eq!(backfilled.sprint_logs.len(), 1);
                assert_eq!(backfilled.sprint_logs[0].sprint_number, 1);

                let already_logged = backend.get_card(already_logged_id).unwrap().unwrap();
                assert_eq!(
                    already_logged.sprint_logs, already_logged_before,
                    "card with existing logs must be untouched"
                );

                let no_sprint = backend.get_card(no_sprint_id).unwrap().unwrap();
                assert!(
                    no_sprint.sprint_logs.is_empty(),
                    "card without sprint_id must remain unlogged"
                );
            }
        }
    };
}

migrate_sprint_logs_tests!(json_backend, open_json_ctx());
migrate_sprint_logs_tests!(sqlite_backend, open_sqlite_ctx());

async fn open_fault_ctx() -> (KanbanContext, Arc<FaultInjectingBackend>) {
    let inner: Arc<dyn KanbanBackend> = Arc::new(InMemoryStore::new());
    let fault = Arc::new(FaultInjectingBackend::new(inner));
    let backend: Arc<dyn KanbanBackend> = fault.clone();
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    (ctx, fault)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_sprint_logs_preserves_undo_stack_when_the_first_write_fails() {
    let (mut ctx, fault) = open_fault_ctx().await;
    let backend = ctx.backend();

    let board = Board::new("B", Some("TST"));
    let col = Column::new(board.id, "Col", 0);
    let sprint = Sprint::new(board.id, 1, None, Some("Alpha"));
    let sprint_id = sprint.id;
    let mut card = Card::new(board.id, col.id, "Card", 0);
    card.sprint_id = Some(sprint_id);
    let card_id = card.id;
    backend.upsert_board(board.clone()).unwrap();
    backend.upsert_column(col.clone()).unwrap();
    backend.upsert_sprint(sprint).unwrap();
    backend.upsert_card(card).unwrap();

    ctx.create_column(board.id, "Another".into(), None).unwrap();
    assert!(
        UndoOperations::can_undo(&ctx),
        "setup: undo stack must be primed"
    );

    fault.fail_upsert_card_after(0, KanbanError::Database("disk I/O error".into()));

    let err = ctx.migrate_sprint_logs().unwrap_err();

    assert!(
        !err.is_unsupported(),
        "a real I/O failure must reach the loud path, got {err:?}"
    );
    assert!(
        UndoOperations::can_undo(&ctx),
        "undo stack must survive a migration that wrote nothing"
    );
    let stored = fault.inner().get_card(card_id).unwrap().unwrap();
    assert!(
        stored.sprint_logs.is_empty(),
        "no card may be mutated when the first write fails"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_sprint_logs_reports_a_declining_backend_as_a_safe_no_op() {
    let (mut ctx, fault) = open_fault_ctx().await;
    let backend = ctx.backend();

    let board = Board::new("B", Some("TST"));
    let col = Column::new(board.id, "Col", 0);
    let sprint = Sprint::new(board.id, 1, None, Some("Alpha"));
    let sprint_id = sprint.id;
    let mut card = Card::new(board.id, col.id, "Card", 0);
    card.sprint_id = Some(sprint_id);
    let card_id = card.id;
    backend.upsert_board(board.clone()).unwrap();
    backend.upsert_column(col.clone()).unwrap();
    backend.upsert_sprint(sprint).unwrap();
    backend.upsert_card(card).unwrap();

    ctx.create_column(board.id, "Another".into(), None).unwrap();
    assert!(
        UndoOperations::can_undo(&ctx),
        "setup: undo stack must be primed"
    );

    fault.fail_upsert_card_after(0, KanbanError::unsupported("upsert_card"));

    let err = ctx.migrate_sprint_logs().unwrap_err();

    assert!(
        err.is_unsupported(),
        "expected a safe-no-op signal, got {err:?}"
    );
    assert!(
        UndoOperations::can_undo(&ctx),
        "undo stack must survive a migration that wrote nothing"
    );
    let stored = fault.inner().get_card(card_id).unwrap().unwrap();
    assert!(
        stored.sprint_logs.is_empty(),
        "no card may be mutated when the backend cannot write"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_sprint_logs_escalates_loudly_once_a_partial_write_has_landed() {
    let (mut ctx, fault) = open_fault_ctx().await;
    let backend = ctx.backend();

    let board = Board::new("B", Some("TST"));
    let col = Column::new(board.id, "Col", 0);
    let sprint = Sprint::new(board.id, 1, None, Some("Alpha"));
    let sprint_id = sprint.id;
    let mut card1 = Card::new(board.id, col.id, "Card 1", 0);
    card1.sprint_id = Some(sprint_id);
    let mut card2 = Card::new(board.id, col.id, "Card 2", 1);
    card2.sprint_id = Some(sprint_id);
    let card2_id = card2.id;
    backend.upsert_board(board.clone()).unwrap();
    backend.upsert_column(col).unwrap();
    backend.upsert_sprint(sprint).unwrap();
    backend.upsert_card(card1).unwrap();
    backend.upsert_card(card2).unwrap();

    ctx.create_column(board.id, "Another".into(), None).unwrap();
    assert!(
        UndoOperations::can_undo(&ctx),
        "setup: undo stack must be primed"
    );

    fault.fail_upsert_card_after(1, KanbanError::unsupported("upsert_card"));

    let err = ctx.migrate_sprint_logs().unwrap_err();

    assert!(
        !err.is_unsupported(),
        "escalation must fire even when the underlying fault was Unsupported, got {err:?}"
    );
    assert!(
        UndoOperations::can_undo(&ctx),
        "undo stack must still be intact — it must only ever be cleared on full success"
    );
    let stored = fault.inner().get_card(card2_id).unwrap().unwrap();
    assert!(
        stored.sprint_logs.is_empty(),
        "the card whose write failed must be unmigrated"
    );
}
