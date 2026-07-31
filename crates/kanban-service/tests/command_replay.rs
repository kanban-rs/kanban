//! Replay determinism: re-executing the recorded command log from a
//! clean baseline reproduces the same business state. Compares ids,
//! content, structure, and relationships — not `updated_at`, which
//! drifts because some model methods stamp `Utc::now()` on each call.

use kanban_backend_memory::InMemoryStore;
use kanban_domain::commands::CommandContext;
use kanban_domain::data_store::DataStore;
use kanban_domain::{KanbanOperations, KanbanResult, Snapshot};
use kanban_service::KanbanContext;
use std::sync::Arc;

async fn make_ctx() -> KanbanContext {
    KanbanContext::open(
        Arc::new(InMemoryStore::new()),
        kanban_core::AppConfig::default(),
    )
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_replay_from_baseline_reproduces_state() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;

    let board = ctx.create_board("B".into(), Some("KAN".into()))?;
    let col_todo = ctx.create_column(board.id, "TODO".into(), None)?;
    let col_done = ctx.create_column(board.id, "Done".into(), None)?;
    let card1 = ctx.create_card(board.id, col_todo.id, "Card 1".into(), Default::default())?;
    let _card2 = ctx.create_card(board.id, col_todo.id, "Card 2".into(), Default::default())?;
    ctx.move_card(card1.id, col_done.id, Some(0))?;
    let sprint = ctx.create_sprint(board.id, None, None)?;
    ctx.assign_card_to_sprint(card1.id, sprint.id)?;

    let original = ctx.snapshot()?;
    let backend = ctx.backend();
    let (batches, count) = backend.load_all_batches()?;
    assert!(count > 0, "should have recorded at least one command batch");

    let replay_backend = Arc::new(InMemoryStore::new());
    replay_backend.apply_snapshot(Snapshot::new())?;
    {
        let cmd_ctx = CommandContext {
            store: replay_backend.as_ref() as &dyn DataStore,
        };
        for batch in &batches {
            for cmd in &batch.commands {
                cmd.execute(&cmd_ctx)?;
            }
        }
    }
    let replayed = replay_backend.snapshot()?;

    assert_eq!(
        original.boards.len(),
        replayed.boards.len(),
        "board count must match"
    );
    for (a, b) in original.boards.iter().zip(&replayed.boards) {
        assert_eq!(a.id, b.id, "board id must match");
        assert_eq!(a.name, b.name, "board name must match");
        assert_eq!(a.card_prefix, b.card_prefix, "board card_prefix must match");
        assert_eq!(a.position, b.position, "board position must match");
        assert_eq!(
            a.card_counter, b.card_counter,
            "board card_counter must match"
        );
    }

    assert_eq!(
        original.columns.len(),
        replayed.columns.len(),
        "column count must match"
    );
    let mut orig_cols: Vec<_> = original.columns.iter().collect();
    orig_cols.sort_by_key(|c| c.id);
    let mut rep_cols: Vec<_> = replayed.columns.iter().collect();
    rep_cols.sort_by_key(|c| c.id);
    for (a, b) in orig_cols.iter().zip(&rep_cols) {
        assert_eq!(a.id, b.id, "column id must match");
        assert_eq!(a.name, b.name, "column name must match for {}", a.id);
        assert_eq!(
            a.board_id, b.board_id,
            "column board_id must match for {}",
            a.id
        );
        assert_eq!(
            a.position, b.position,
            "column position must match for {}",
            a.id
        );
    }

    assert_eq!(
        original.cards.len(),
        replayed.cards.len(),
        "card count must match"
    );
    let mut orig_cards: Vec<_> = original.cards.iter().collect();
    orig_cards.sort_by_key(|c| c.id);
    let mut rep_cards: Vec<_> = replayed.cards.iter().collect();
    rep_cards.sort_by_key(|c| c.id);
    for (a, b) in orig_cards.iter().zip(&rep_cards) {
        assert_eq!(a.id, b.id, "card id must match");
        assert_eq!(a.title, b.title, "card title must match for {}", a.id);
        assert_eq!(
            a.column_id, b.column_id,
            "card column must match for {}",
            a.id
        );
        assert_eq!(
            a.position, b.position,
            "card position must match for {}",
            a.id
        );
        assert_eq!(
            a.sprint_id, b.sprint_id,
            "card sprint must match for {}",
            a.id
        );
    }

    assert_eq!(
        original.sprints.len(),
        replayed.sprints.len(),
        "sprint count must match"
    );
    let mut orig_sprints: Vec<_> = original.sprints.iter().collect();
    orig_sprints.sort_by_key(|s| s.id);
    let mut rep_sprints: Vec<_> = replayed.sprints.iter().collect();
    rep_sprints.sort_by_key(|s| s.id);
    for (a, b) in orig_sprints.iter().zip(&rep_sprints) {
        assert_eq!(a.id, b.id, "sprint id must match");
        assert_eq!(a.board_id, b.board_id, "sprint board_id must match");
        assert_eq!(a.sprint_number, b.sprint_number, "sprint number must match");
        assert_eq!(a.status, b.status, "sprint status must match");
    }

    Ok(())
}
