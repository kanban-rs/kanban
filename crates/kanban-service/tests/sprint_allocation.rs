//! Sprint numbers are allocated from the shared prefix row, not from the
//! board's private `sprint_counters` map.
//!
//! The mirror of `prefix_allocation.rs` on the sprint axis. Same defect, same
//! shape: per-board counters let two boards sharing a prefix each hand out
//! "sprint 1", and a card's sibling concept must not be governed by a
//! different rule than the card.
//!
//! Assertions are on the ROW VALUE wherever the returned number alone would
//! pass identically against the legacy map.

use kanban_core::AppConfig;
use kanban_domain::KanbanOperations;
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

/// An absent row and a row at zero both mean "nothing allocated from this
/// namespace yet".
fn sprint_counter(ctx: &KanbanContext, name: &str) -> u32 {
    ctx.backend()
        .get_prefix(name)
        .unwrap()
        .map_or(0, |p| p.sprint_counter)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sprint_numbers_are_drawn_from_the_prefix_row_counter() {
    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), None).unwrap();
    let before = sprint_counter(&c, "sprint");

    c.create_sprint(board.id, None, None).unwrap();
    let after_one = sprint_counter(&c, "sprint");
    c.create_sprint(board.id, None, None).unwrap();
    let after_two = sprint_counter(&c, "sprint");

    assert_eq!(
        (after_one, after_two),
        (before + 1, before + 2),
        "each create must advance the PREFIX ROW's sprint counter; advancing \
         only board.sprint_counters would leave this at zero"
    );
}

/// The invariant this card exists for, on the sprint axis.
#[tokio::test(flavor = "multi_thread")]
async fn test_two_boards_sharing_a_sprint_prefix_never_mint_the_same_number() {
    use kanban_domain::{BoardUpdate, FieldUpdate};

    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let a = c.create_board("A".into(), None).unwrap();
    let b = c.create_board("B".into(), None).unwrap();
    for id in [a.id, b.id] {
        c.update_board(
            id,
            BoardUpdate {
                sprint_prefix: FieldUpdate::Set("REL".into()),
                ..Default::default()
            },
        )
        .unwrap();
    }

    let first = c.create_sprint(a.id, None, None).unwrap();
    let second = c.create_sprint(b.id, None, None).unwrap();

    assert_ne!(
        first.sprint_number, second.sprint_number,
        "one namespace, one counter; per-board counters give both number 1"
    );
    assert_eq!(sprint_counter(&c, "rel"), 2, "both advanced the one row");
}

/// Casing separates display from matching here exactly as it does for cards.
#[tokio::test(flavor = "multi_thread")]
async fn test_differently_cased_sprint_prefixes_share_one_namespace() {
    use kanban_domain::{BoardUpdate, FieldUpdate};

    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let a = c.create_board("A".into(), None).unwrap();
    let b = c.create_board("B".into(), None).unwrap();
    for (id, spelling) in [(a.id, "REL"), (b.id, "rel")] {
        c.update_board(
            id,
            BoardUpdate {
                sprint_prefix: FieldUpdate::Set(spelling.into()),
                ..Default::default()
            },
        )
        .unwrap();
    }

    let first = c.create_sprint(a.id, None, None).unwrap();
    let second = c.create_sprint(b.id, None, None).unwrap();

    assert_ne!(first.sprint_number, second.sprint_number);
    assert_eq!(
        first.prefix.as_deref(),
        Some("REL"),
        "each sprint keeps ITS board's configured casing"
    );
    assert_eq!(second.prefix.as_deref(), Some("rel"));
    assert_eq!(sprint_counter(&c, "rel"), 2);
}

/// A sprint whose own prefix overrides its board's allocates from THAT
/// namespace, leaving the board's counter untouched.
#[tokio::test(flavor = "multi_thread")]
async fn test_a_sprint_prefix_override_allocates_from_its_own_namespace() {
    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), None).unwrap();
    let board_before = sprint_counter(&c, "sprint");

    let sprint = c
        .create_sprint(board.id, Some("REL".to_string()), None)
        .unwrap();

    assert_eq!(sprint.prefix.as_deref(), Some("REL"));
    assert_eq!(
        sprint_counter(&c, "sprint"),
        board_before,
        "the board's namespace must not advance, or it permanently skips a number"
    );
    assert_eq!(sprint_counter(&c, "rel"), 1);
}

/// KAN-1216 removes `board.sprint_counters` only once this read path has
/// proven itself. Until then both move together, and deleting this test is
/// what that card does.
#[tokio::test(flavor = "multi_thread")]
async fn test_legacy_board_sprint_counter_still_moves_in_lockstep() {
    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), None).unwrap();
    c.create_sprint(board.id, None, None).unwrap();

    assert_eq!(
        c.get_board(board.id)
            .unwrap()
            .unwrap()
            .get_sprint_counter("sprint"),
        Some(2),
        "the legacy map stores the NEXT number and stays in sync until KAN-1216 \
         removes it"
    );
}

/// Numbers must be contiguous across the point where allocation moved onto the
/// prefix row. A row seeded with the legacy next-to-hand-out instead of the
/// last-used would make this skip.
#[tokio::test(flavor = "multi_thread")]
async fn test_sprint_numbering_is_contiguous_across_the_allocation_switch() {
    let dir = TempDir::new().unwrap();
    let mut c = ctx(&dir.path().join("s.db")).await;

    let board = c.create_board("B".into(), None).unwrap();
    let numbers: Vec<u32> = (0..3)
        .map(|_| c.create_sprint(board.id, None, None).unwrap().sprint_number)
        .collect();

    assert_eq!(
        numbers,
        vec![1, 2, 3],
        "sprint numbering starts at 1 and never skips"
    );
}
