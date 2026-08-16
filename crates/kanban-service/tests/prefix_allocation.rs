//! Card numbers are allocated from the prefix row, not from the board.
//!
//! Every assertion here is on the `prefixes` ROW VALUE rather than on the
//! returned `card_number`. Asserting the returned number only proves it went
//! up, which passes identically against `boards.card_counter` or a
//! `MAX(card_number)` scan -- the two implementations this card replaces.

use kanban_core::AppConfig;
use kanban_domain::{CreateCardOptions, DataStore, KanbanOperations};
use kanban_service::KanbanContext;
use std::sync::Arc;
use tempfile::TempDir;

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
        .unwrap_or_else(|| panic!("no prefix row named {name}"))
        .card_counter
}

#[tokio::test(flavor = "multi_thread")]
async fn test_card_numbers_are_drawn_from_the_prefix_row_counter() {
    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = c.create_column(board.id, "Todo".into(), None).unwrap();

    let before = counter(&c, "kan");
    let one = c
        .create_card(board.id, col.id, "one".into(), CreateCardOptions::default())
        .unwrap();
    let after_one = counter(&c, "kan");
    let two = c
        .create_card(board.id, col.id, "two".into(), CreateCardOptions::default())
        .unwrap();
    let after_two = counter(&c, "kan");

    assert_eq!(
        (after_one, after_two),
        (before + 1, before + 2),
        "each create must advance the PREFIX ROW's counter; advancing only \
         boards.card_counter would leave this unchanged"
    );
    assert_eq!(one.prefix, "kan");
    assert_eq!(two.prefix, "kan");
    assert_ne!(one.card_number, two.card_number);
}

/// The invariant the whole epic exists for. Two boards sharing a namespace
/// draw from ONE counter, so they cannot mint the same identifier -- which is
/// exactly what per-board counters allow today.
#[tokio::test(flavor = "multi_thread")]
async fn test_two_boards_sharing_a_prefix_never_mint_the_same_identifier() {
    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let a = c.create_board("A".into(), Some("SHARED".into())).unwrap();
    let b = c.create_board("B".into(), Some("shared".into())).unwrap();
    let col_a = c.create_column(a.id, "Todo".into(), None).unwrap();
    let col_b = c.create_column(b.id, "Todo".into(), None).unwrap();

    let card_a = c
        .create_card(a.id, col_a.id, "a".into(), CreateCardOptions::default())
        .unwrap();
    let card_b = c
        .create_card(b.id, col_b.id, "b".into(), CreateCardOptions::default())
        .unwrap();

    assert_eq!(
        card_a.prefix, card_b.prefix,
        "one namespace, spelled two ways"
    );
    assert_ne!(
        (card_a.prefix.as_str(), card_a.card_number),
        (card_b.prefix.as_str(), card_b.card_number),
        "cards on different boards sharing a namespace must not collide; this \
         is the defect that let KAN-1 name two different cards"
    );
}

/// A card created into a sprint that overrides its board's prefix must draw
/// from THAT namespace's counter, not the board's.
#[tokio::test(flavor = "multi_thread")]
async fn test_a_sprint_override_allocates_from_the_sprints_namespace() {
    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = c.create_column(board.id, "Todo".into(), None).unwrap();
    let sprint = c.create_sprint(board.id, None, None).unwrap();
    c.update_sprint(
        sprint.id,
        kanban_domain::SprintUpdate {
            card_prefix: kanban_domain::FieldUpdate::Set("AUTH".into()),
            ..Default::default()
        },
    )
    .unwrap();

    let board_before = counter(&c, "kan");
    let card = c
        .create_card(
            board.id,
            col.id,
            "in sprint".into(),
            CreateCardOptions {
                sprint_id: Some(sprint.id),
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(card.prefix, "auth", "stamped with the sprint's namespace");
    assert_eq!(
        counter(&c, "kan"),
        board_before,
        "allocating from the sprint's namespace must NOT advance the board's \
         counter, or the board permanently skips numbers"
    );
    assert_eq!(
        counter(&c, "auth"),
        1,
        "the sprint's namespace is what advanced"
    );
}

/// KAN-1216 removes `boards.card_counter` only once KAN-1215 reads through the
/// prefix row. Until then both move together, and deleting this test is what
/// that card does.
#[tokio::test(flavor = "multi_thread")]
async fn test_legacy_board_counter_still_moves_in_lockstep() {
    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = c.create_column(board.id, "Todo".into(), None).unwrap();
    let before = c.get_board(board.id).unwrap().unwrap().card_counter;

    c.create_card(board.id, col.id, "one".into(), CreateCardOptions::default())
        .unwrap();

    assert_eq!(
        c.get_board(board.id).unwrap().unwrap().card_counter,
        before + 1,
        "the legacy counter stays in sync until KAN-1216 removes it"
    );
}
