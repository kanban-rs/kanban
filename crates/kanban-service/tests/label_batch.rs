//! Batch label operations against the in-memory backend, exercising the
//! registry semantics from issue #966: assign auto-registers the label
//! (empty metadata) without clobbering an existing one, remove leaves the
//! registry intact and leaves untagged cards untouched, and both run as one
//! undo unit, all-or-nothing per card batch.

use kanban_backend_memory::InMemoryStore;
use kanban_core::AppConfig;
use kanban_domain::{DomainError, KanbanError, KanbanOperations, Label, UndoOperations};
use kanban_service::KanbanContext;
use std::sync::Arc;
use uuid::Uuid;

async fn make_ctx() -> KanbanContext {
    KanbanContext::open(Arc::new(InMemoryStore::new()), AppConfig::default())
        .await
        .unwrap()
}

/// A context with one board, one column, and two fresh cards.
async fn seeded() -> (KanbanContext, Uuid, Uuid) {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None).unwrap();
    let col = ctx.create_column(board.id, "C".into(), None).unwrap();
    let a = ctx
        .create_card(board.id, col.id, "A".into(), Default::default())
        .unwrap();
    let b = ctx
        .create_card(board.id, col.id, "B".into(), Default::default())
        .unwrap();
    (ctx, a.id, b.id)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_assign_registers_missing_label_and_tags_all_cards() {
    let (mut ctx, a, b) = seeded().await;

    let (count, _inv) = ctx
        .assign_label_to_cards_impl("urgent".to_string(), vec![a, b])
        .unwrap();

    assert_eq!(count, 2, "both cards must report as applied");

    let label = ctx
        .get_label("urgent")
        .unwrap()
        .expect("label auto-registered");
    assert_eq!(label.name, "urgent");
    assert!(label.color.is_none(), "auto-registration carries no color");
    assert!(
        label.description.is_none(),
        "auto-registration carries no description"
    );

    for id in [a, b] {
        let card = ctx.get_card(id).unwrap().unwrap();
        assert_eq!(card.labels, vec!["urgent".to_string()]);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_assign_never_clobbers_existing_label_metadata() {
    let (mut ctx, card_a, _b) = seeded().await;

    ctx.upsert_label(Label::new(
        "backend",
        Some("#aa0000".to_string()),
        Some("Server-side work".to_string()),
    ))
    .unwrap();

    ctx.assign_label_to_cards_impl("backend".to_string(), vec![card_a])
        .unwrap();

    let label = ctx.get_label("backend").unwrap().expect("registry entry");
    assert_eq!(
        label.color.as_deref(),
        Some("#aa0000"),
        "existing label color must survive the assign"
    );
    assert_eq!(
        label.description.as_deref(),
        Some("Server-side work"),
        "existing label description must survive the assign"
    );

    let card = ctx.get_card(card_a).unwrap().unwrap();
    assert_eq!(card.labels, vec!["backend".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_assign_twice_does_not_duplicate_the_tag() {
    let (mut ctx, card_a, card_b) = seeded().await;

    ctx.assign_label_to_cards_impl("dup".to_string(), vec![card_a])
        .unwrap();
    let (count, _inv) = ctx
        .assign_label_to_cards_impl("dup".to_string(), vec![card_a, card_b])
        .unwrap();

    assert_eq!(count, 2);
    let card = ctx.get_card(card_a).unwrap().unwrap();
    assert_eq!(
        card.labels,
        vec!["dup".to_string()],
        "re-assigning a card must not append the label again"
    );
    assert_eq!(
        ctx.get_card(card_b).unwrap().unwrap().labels,
        vec!["dup".to_string()]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_assign_fails_atomically_when_any_card_is_missing() {
    let (mut ctx, card_a, _b) = seeded().await;
    let missing = Uuid::new_v4();

    let err = ctx
        .assign_label_to_cards_impl("gone".to_string(), vec![card_a, missing])
        .unwrap_err();

    assert!(
        matches!(
            &err,
            KanbanError::Domain(DomainError::NotFound { entity, .. }) if *entity == "Card"
        ),
        "expected a Card not-found error, got: {err:?}"
    );

    let card = ctx.get_card(card_a).unwrap().unwrap();
    assert!(
        card.labels.is_empty(),
        "the valid card must not be tagged when the batch fails"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remove_keeps_registry_strips_only_the_named_label() {
    let (mut ctx, card_a, card_b) = seeded().await;

    // Give card a two labels: "keep" and "drop".
    ctx.assign_label_to_cards_impl("keep".to_string(), vec![card_a])
        .unwrap();
    ctx.assign_label_to_cards_impl("drop".to_string(), vec![card_a, card_b])
        .unwrap();

    ctx.remove_label_from_cards_impl("drop".to_string(), vec![card_a, card_b])
        .unwrap();

    let label = ctx
        .get_label("drop")
        .unwrap()
        .expect("delete is out of scope; remove must not delete the registry entry");
    assert_eq!(label.name, "drop");

    let kept = ctx.get_card(card_a).unwrap().unwrap().labels;
    assert_eq!(
        kept,
        vec!["keep".to_string()],
        "removing 'drop' must leave 'keep' intact"
    );
    assert!(ctx.get_card(card_b).unwrap().unwrap().labels.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remove_from_a_card_that_does_not_carry_the_label_is_a_noop() {
    let (mut ctx, card_a, card_b) = seeded().await;

    ctx.assign_label_to_cards_impl("solo".to_string(), vec![card_a])
        .unwrap();
    let before = ctx.get_card(card_b).unwrap().unwrap();

    let (count, _inv) = ctx
        .remove_label_from_cards_impl("solo".to_string(), vec![card_b])
        .unwrap();

    assert_eq!(count, 1);
    let after = ctx.get_card(card_b).unwrap().unwrap();
    assert_eq!(
        before, after,
        "a card not carrying the label must come back untouched"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_undo_assign_reverts_cards_and_registry_in_one_step() {
    let (mut ctx, card_a, card_b) = seeded().await;

    ctx.assign_label_to_cards_impl("tmp".to_string(), vec![card_a, card_b])
        .unwrap();

    ctx.undo().unwrap();

    for id in [card_a, card_b] {
        assert!(ctx.get_card(id).unwrap().unwrap().labels.is_empty());
    }
    assert!(
        ctx.get_label("tmp").unwrap().is_none(),
        "the auto-registered label must be un-registered by the undo"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_label_strips_it_from_cards_so_a_save_cannot_resurrect_it() {
    let (mut ctx, card_a, card_b) = seeded().await;

    // Both cards carry "bye"; card_a also carries "stay".
    ctx.assign_label_to_cards_impl("bye".to_string(), vec![card_a, card_b])
        .unwrap();
    ctx.assign_label_to_cards_impl("stay".to_string(), vec![card_a]).unwrap();
    assert!(ctx.get_label("bye").unwrap().is_some());

    ctx.delete_label("bye".to_string()).unwrap();

    // The registry entry is gone ...
    assert!(
        ctx.get_label("bye").unwrap().is_none(),
        "the deleted label must be un-registered"
    );
    // ... and the name is stripped from every card that carried it. The save
    // path auto-registers any card label name missing from the registry, so a
    // card left with the name would resurrect it on its next save; stripping it
    // from the cards is what keeps the delete permanent.
    for id in [card_a, card_b] {
        assert!(
            !ctx.get_card(id).unwrap().unwrap().labels.contains(&"bye".to_string()),
            "delete must strip 'bye' from the card"
        );
    }
    // A different label on the same card survives the delete.
    assert_eq!(
        ctx.get_card(card_a).unwrap().unwrap().labels,
        vec!["stay".to_string()]
    );

    // Re-saving card_a must not resurrect the label: the card no longer carries
    // it, so the auto-register step has nothing to re-add.
    ctx.update_card(
        card_a,
        kanban_domain::CardUpdate {
            title: Some("renamed".to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        ctx.get_label("bye").unwrap().is_none(),
        "a later save must not resurrect a deleted label"
    );
    assert!(
        ctx.get_label("stay").unwrap().is_some(),
        "the retained label must still be registered"
    );
}
