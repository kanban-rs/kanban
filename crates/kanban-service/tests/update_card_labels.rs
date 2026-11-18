//! L3: service-tier tests for card label updates (Set, Clear, undo) and the
//! PATCH wire semantics (set/clear on `card.labels`).

use kanban_backend_memory::InMemoryStore;
use kanban_domain::{CardUpdate, FieldUpdate, KanbanOperations, KanbanResult, UndoOperations};
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
async fn test_update_card_labels_sets_clears_and_undoes() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;

    // Starting state: no labels.
    let read = ctx.get_card(card.id)?.unwrap();
    assert_eq!(
        read.labels,
        Vec::<String>::new(),
        "fresh card has no labels"
    );

    // Step 1: set labels = ["y"]
    let updated = ctx.update_card(
        card.id,
        CardUpdate {
            labels: FieldUpdate::Set(vec!["y".to_string()]),
            ..Default::default()
        },
    )?;
    assert_eq!(
        updated.labels,
        vec!["y".to_string()],
        "Set([y]) produces label [y]"
    );

    // Step 2: clear labels
    let cleared = ctx.update_card(
        card.id,
        CardUpdate {
            labels: FieldUpdate::Clear,
            ..Default::default()
        },
    )?;
    assert_eq!(
        cleared.labels,
        Vec::<String>::new(),
        "Clear removes all labels"
    );

    // Step 3: set labels = ["z", "w"]
    let set2 = ctx.update_card(
        card.id,
        CardUpdate {
            labels: FieldUpdate::Set(vec!["z".to_string(), "w".to_string()]),
            ..Default::default()
        },
    )?;
    assert_eq!(
        set2.labels,
        vec!["z".to_string(), "w".to_string()],
        "Set([z,w]) produces labels [z,w] in order"
    );

    // Undo Step 3 → back to []
    let undone3 = ctx.undo()?.expect("undo step 3 applied");
    let read3 = ctx.get_card(card.id)?.unwrap();
    let _ = undone3; // Invalidation returned; not asserted here.
    assert_eq!(
        read3.labels,
        Vec::<String>::new(),
        "undo Set([z,w]) restores []"
    );

    // Undo Step 2 → back to ["y"]
    let undone2 = ctx.undo()?.expect("undo step 2 applied");
    let read2 = ctx.get_card(card.id)?.unwrap();
    let _ = undone2;
    assert_eq!(
        read2.labels,
        vec!["y".to_string()],
        "undo Clear restores [y]"
    );

    // Undo Step 1 → back to [] (original)
    let undone1 = ctx.undo()?.expect("undo step 1 applied");
    let read1 = ctx.get_card(card.id)?.unwrap();
    let _ = undone1;
    assert_eq!(
        read1.labels,
        Vec::<String>::new(),
        "undo Set([y]) restores []"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_api_patch_labels_set_then_clear_updates_card() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;

    // PATCH {"labels": {"set": ["a"]}} — maps to FieldUpdate::Set(["a"])
    let updated = ctx.update_card(
        card.id,
        CardUpdate {
            labels: FieldUpdate::Set(vec!["a".to_string()]),
            ..Default::default()
        },
    )?;
    assert_eq!(
        updated.labels,
        vec!["a".to_string()],
        "PATCH set labels=[a] yields card.labels==[a]"
    );

    // PATCH {"labels": {"clear": true}} — maps to FieldUpdate::Clear
    let cleared = ctx.update_card(
        card.id,
        CardUpdate {
            labels: FieldUpdate::Clear,
            ..Default::default()
        },
    )?;
    assert_eq!(
        cleared.labels,
        Vec::<String>::new(),
        "PATCH clear labels yields card.labels==[]"
    );

    Ok(())
}
