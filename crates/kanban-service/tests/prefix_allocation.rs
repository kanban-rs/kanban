//! Card numbers are allocated from the prefix row, not from the board.
//!
//! Every assertion here is on the `prefixes` ROW VALUE rather than on the
//! returned `card_number`. Asserting the returned number only proves it went
//! up, which passes identically against `boards.card_counter` or a
//! `MAX(card_number)` scan -- the two implementations this card replaces.

use kanban_core::AppConfig;
use kanban_domain::{CreateCardOptions, KanbanOperations};
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

/// An absent row and a row at zero mean the same thing: nothing has been
/// allocated from that namespace yet. The row is created on first allocation,
/// so reading before any card exists must not be an error.
fn counter(ctx: &KanbanContext, name: &str) -> u32 {
    ctx.backend()
        .get_prefix(name)
        .unwrap()
        .map_or(0, |p| p.card_counter)
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
    assert_eq!(one.prefix, "KAN");
    assert_eq!(two.prefix, "KAN");
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
        card_a.prefix.to_lowercase(),
        card_b.prefix.to_lowercase(),
        "one namespace, spelled two ways -- each card keeps its own board's casing"
    );
    assert_ne!(
        (card_a.prefix.to_lowercase(), card_a.card_number),
        (card_b.prefix.to_lowercase(), card_b.card_number),
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

    assert_eq!(
        card.prefix, "AUTH",
        "stamped with the sprint's namespace, casing kept"
    );
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

/// Allocation moved ahead of `CreateCard::execute`, but the WIP check lives
/// inside it, so a rejected create used to bump the counter and then fail.
///
/// The counter is the discriminating assertion, not the error: the create
/// correctly fails either way. What must not happen is a number being reserved
/// for a card that was never created.
#[tokio::test(flavor = "multi_thread")]
async fn test_a_wip_rejected_create_does_not_burn_a_card_number() {
    use kanban_domain::{ColumnUpdate, FieldUpdate};

    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = c.create_column(board.id, "Todo".into(), None).unwrap();
    c.update_column(
        col.id,
        ColumnUpdate {
            wip_limit: FieldUpdate::Set(1),
            ..Default::default()
        },
    )
    .unwrap();

    let first = c
        .create_card(board.id, col.id, "one".into(), CreateCardOptions::default())
        .unwrap();
    let before = counter(&c, "kan");

    let rejected = c.create_card(board.id, col.id, "two".into(), CreateCardOptions::default());
    assert!(rejected.is_err(), "the column is full, so this must fail");

    assert_eq!(
        counter(&c, "kan"),
        before,
        "a create that never produced a card must not consume a number"
    );

    // And numbering stays contiguous once the column has room again.
    c.delete_card(first.id).unwrap();
    let next = c
        .create_card(
            board.id,
            col.id,
            "three".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    assert_eq!(
        next.card_number,
        first.card_number + 1,
        "the rejected create must leave no gap"
    );
}

/// A subcard is created by a different command on a different layer
/// (`CreateSubcardCommand`, inside the domain). Before this card it minted from
/// `board.card_counter` while ordinary cards minted from the prefix row, so the
/// two drew from INDEPENDENT counters. They do not necessarily collide on the
/// next create -- they drift, and drift is what lets them collide later. The
/// discriminating assertion below is the counter, not the identifiers.
#[tokio::test(flavor = "multi_thread")]
async fn test_a_subcard_allocates_from_the_same_counter_as_its_siblings() {
    use kanban_domain::commands::{Command, CreateSubcardCommand, DependencyCommand};
    use uuid::Uuid;

    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = c.create_column(board.id, "Todo".into(), None).unwrap();
    let parent = c
        .create_card(
            board.id,
            col.id,
            "parent".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    let subcard_id = Uuid::new_v4();
    c.execute(vec![Command::Dependency(DependencyCommand::CreateSubcard(
        CreateSubcardCommand {
            id: subcard_id,
            parent_id: parent.id,
            board_id: board.id,
            column_id: col.id,
            title: "sub".into(),
            description: None,
            position: 1,
            default_card_prefix: "task".to_string(),
        },
    ))])
    .unwrap();

    let sub = c.get_card(subcard_id).unwrap().unwrap();
    assert_eq!(
        sub.prefix, "KAN",
        "a subcard belongs to its board's namespace, casing included"
    );
    assert_ne!(
        (sub.prefix.as_str(), sub.card_number),
        (parent.prefix.as_str(), parent.card_number),
        "a subcard must not collide with its own parent"
    );
    assert_eq!(
        counter(&c, "kan"),
        2,
        "both creates advanced ONE counter; a second counter would leave this at 1"
    );
}

/// Single-identifier and batch resolution must mean the same thing by
/// `KAN-5`. They used different rules: `card get` reads the stored prefix
/// while `resolve_card_ids` re-derived it through the card's board, so a board
/// rename made them disagree -- one finding the card under its old identifier
/// and the other under the new one.
#[tokio::test(flavor = "multi_thread")]
async fn test_batch_and_single_resolution_agree_after_a_board_rename() {
    use kanban_domain::{BoardUpdate, FieldUpdate};

    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), Some("OLD".into())).unwrap();
    let col = c.create_column(board.id, "Todo".into(), None).unwrap();
    let card = c
        .create_card(board.id, col.id, "one".into(), CreateCardOptions::default())
        .unwrap();
    let ident = format!("{}-{}", card.prefix, card.card_number);

    c.update_board(
        board.id,
        BoardUpdate {
            card_prefix: FieldUpdate::Set("NEW".into()),
            ..Default::default()
        },
    )
    .unwrap();

    let single = c.find_cards_by_identifier(&ident).unwrap();
    let batch = c.resolve_card_ids(std::slice::from_ref(&ident)).unwrap();

    assert_eq!(single.len(), 1, "{ident} still resolves after the rename");
    assert_eq!(
        batch,
        vec![card.id],
        "batch resolution must agree with single resolution about {ident}"
    );

    // And neither answers to the board's NEW prefix, because the card was
    // never minted under it.
    let renamed = format!("new-{}", card.card_number);
    assert!(c.find_cards_by_identifier(&renamed).unwrap().is_empty());
    assert!(c.resolve_card_ids(&[renamed]).is_err());
}

/// Casing is a DISPLAY concern; uniqueness is a MATCHING concern. A card stores
/// the prefix as its board was configured, so anything rendering an identifier
/// shows what the user chose.
#[tokio::test(flavor = "multi_thread")]
async fn test_a_card_stores_the_prefix_casing_its_board_was_configured_with() {
    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = c.create_column(board.id, "Todo".into(), None).unwrap();
    let card = c
        .create_card(board.id, col.id, "one".into(), CreateCardOptions::default())
        .unwrap();

    assert_eq!(
        card.prefix, "KAN",
        "the stored prefix keeps the configured casing"
    );

    // ...and survives storage.
    let reloaded = c.get_card(card.id).unwrap().unwrap();
    assert_eq!(reloaded.prefix, "KAN");
}

/// Matching stays case-insensitive regardless of how the prefix was stored, so
/// storing the casing costs nothing at lookup time.
#[tokio::test(flavor = "multi_thread")]
async fn test_lookup_is_case_insensitive_whatever_casing_was_stored() {
    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = c.create_column(board.id, "Todo".into(), None).unwrap();
    let card = c
        .create_card(board.id, col.id, "one".into(), CreateCardOptions::default())
        .unwrap();
    let n = card.card_number;

    for probe in [format!("KAN-{n}"), format!("kan-{n}"), format!("Kan-{n}")] {
        let found = c.find_cards_by_identifier(&probe).unwrap();
        assert_eq!(found.len(), 1, "{probe} must resolve");
        assert_eq!(found[0].id, card.id);
    }

    // Batch resolution must agree, since it matches independently.
    let ident = format!("kan-{n}");
    assert_eq!(
        c.resolve_card_ids(std::slice::from_ref(&ident)).unwrap(),
        vec![card.id]
    );
}

/// Two boards spelling one prefix differently still share ONE namespace: the
/// row name is normalised even though the stamped value is not.
#[tokio::test(flavor = "multi_thread")]
async fn test_differently_cased_boards_still_share_one_namespace() {
    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let a = c.create_board("A".into(), Some("KAN".into())).unwrap();
    let b = c.create_board("B".into(), Some("kan".into())).unwrap();
    let col_a = c.create_column(a.id, "Todo".into(), None).unwrap();
    let col_b = c.create_column(b.id, "Todo".into(), None).unwrap();

    let one = c
        .create_card(a.id, col_a.id, "a".into(), CreateCardOptions::default())
        .unwrap();
    let two = c
        .create_card(b.id, col_b.id, "b".into(), CreateCardOptions::default())
        .unwrap();

    assert_ne!(
        one.card_number, two.card_number,
        "one shared counter, so the numbers differ"
    );
    assert_eq!(one.prefix, "KAN", "each card keeps ITS board's casing");
    assert_eq!(two.prefix, "kan");
    assert_eq!(counter(&c, "kan"), 2, "and both advanced the one row");
}

/// The defect KAN-1248 exists for: a branch name is the identifier a user
/// checks out, and must neither drift when the board is renamed nor change
/// casing.
#[tokio::test(flavor = "multi_thread")]
async fn test_branch_name_keeps_the_stored_prefix_and_its_casing() {
    use kanban_domain::{BoardUpdate, FieldUpdate};

    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = c.create_column(board.id, "Todo".into(), None).unwrap();
    let card = c
        .create_card(
            board.id,
            col.id,
            "Fix the thing".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    let board_now = c.get_board(board.id).unwrap().unwrap();
    assert!(
        card.branch_name(&board_now, &[], "task")
            .starts_with("KAN-"),
        "configured casing preserved, got {}",
        card.branch_name(&board_now, &[], "task")
    );

    c.update_board(
        board.id,
        BoardUpdate {
            card_prefix: FieldUpdate::Set("DEV".into()),
            ..Default::default()
        },
    )
    .unwrap();

    let board_now = c.get_board(board.id).unwrap().unwrap();
    let card = c.get_card(card.id).unwrap().unwrap();
    let branch = card.branch_name(&board_now, &[], "task");
    assert!(
        branch.starts_with("KAN-"),
        "a rename must not move the branch of a card already minted, got {branch}"
    );
}
