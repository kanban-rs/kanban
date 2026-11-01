//! L3: service-tier tests for card tag updates (Set, Clear, undo) and the
//! PATCH wire semantics (set/clear on `card.tags`).

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
async fn test_update_card_tags_sets_clears_and_undoes() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;

    // Starting state: no tags.
    let read = ctx.get_card(card.id)?.unwrap();
    assert_eq!(read.tags, Vec::<String>::new(), "fresh card has no tags");

    // Step 1: set tags = ["y"]
    let updated = ctx.update_card(
        card.id,
        CardUpdate {
            tags: FieldUpdate::Set(vec!["y".to_string()]),
            ..Default::default()
        },
    )?;
    assert_eq!(
        updated.tags,
        vec!["y".to_string()],
        "Set([y]) produces tag [y]"
    );

    // Step 2: clear tags
    let cleared = ctx.update_card(
        card.id,
        CardUpdate {
            tags: FieldUpdate::Clear,
            ..Default::default()
        },
    )?;
    assert_eq!(cleared.tags, Vec::<String>::new(), "Clear removes all tags");

    // Step 3: set tags = ["z", "w"]
    let set2 = ctx.update_card(
        card.id,
        CardUpdate {
            tags: FieldUpdate::Set(vec!["z".to_string(), "w".to_string()]),
            ..Default::default()
        },
    )?;
    assert_eq!(
        set2.tags,
        vec!["z".to_string(), "w".to_string()],
        "Set([z,w]) produces tags [z,w] in order"
    );

    // Undo Step 3 → back to []
    let undone3 = ctx.undo()?.expect("undo step 3 applied");
    let read3 = ctx.get_card(card.id)?.unwrap();
    let _ = undone3; // Invalidation returned; not asserted here.
    assert_eq!(
        read3.tags,
        Vec::<String>::new(),
        "undo Set([z,w]) restores []"
    );

    // Undo Step 2 → back to ["y"]
    let undone2 = ctx.undo()?.expect("undo step 2 applied");
    let read2 = ctx.get_card(card.id)?.unwrap();
    let _ = undone2;
    assert_eq!(read2.tags, vec!["y".to_string()], "undo Clear restores [y]");

    // Undo Step 1 → back to [] (original)
    let undone1 = ctx.undo()?.expect("undo step 1 applied");
    let read1 = ctx.get_card(card.id)?.unwrap();
    let _ = undone1;
    assert_eq!(
        read1.tags,
        Vec::<String>::new(),
        "undo Set([y]) restores []"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_api_patch_tags_set_then_clear_updates_card() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;

    // PATCH {"tags": {"set": ["a"]}} — maps to FieldUpdate::Set(["a"])
    let updated = ctx.update_card(
        card.id,
        CardUpdate {
            tags: FieldUpdate::Set(vec!["a".to_string()]),
            ..Default::default()
        },
    )?;
    assert_eq!(
        updated.tags,
        vec!["a".to_string()],
        "PATCH set tags=[a] yields card.tags==[a]"
    );

    // PATCH {"tags": {"clear": true}} — maps to FieldUpdate::Clear
    let cleared = ctx.update_card(
        card.id,
        CardUpdate {
            tags: FieldUpdate::Clear,
            ..Default::default()
        },
    )?;
    assert_eq!(
        cleared.tags,
        Vec::<String>::new(),
        "PATCH clear tags yields card.tags==[]"
    );

    Ok(())
}
