//! C3b: archived-board descendants are hidden from user-facing cross-board reads
//! (list_cards, find-by-identifier, list_all_*, accessors, resolvers) but
//! preserved on fidelity paths (snapshot, export).

use kanban_domain::{
    CardListFilter, GraphOperations, InMemoryStore, KanbanOperations, KanbanResult, RelatesKind,
};
use kanban_service::{AppConfig, KanbanContext};
use std::sync::Arc;
use uuid::Uuid;

async fn ctx() -> KanbanContext {
    KanbanContext::open(Arc::new(InMemoryStore::new()), AppConfig::default())
        .await
        .unwrap()
}

/// Returns (archived_board_id, archived_card_id, live_card_id).
fn seed(c: &mut KanbanContext) -> KanbanResult<(Uuid, Uuid, Uuid)> {
    let a = c.create_board("A".into(), None)?;
    let a_col = c.create_column(a.id, "Todo".into(), None)?;
    let a_card = c.create_card(a.id, a_col.id, "A-card".into(), Default::default())?;
    let b = c.create_board("B".into(), None)?;
    let b_col = c.create_column(b.id, "Todo".into(), None)?;
    let b_card = c.create_card(b.id, b_col.id, "B-card".into(), Default::default())?;
    c.archive_board(a.id)?;
    Ok((a.id, a_card.id, b_card.id))
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archived_board_cards_hidden_from_list_cards() -> KanbanResult<()> {
    let mut c = ctx().await;
    let (_a, a_card, b_card) = seed(&mut c)?;
    let ids: Vec<_> = c
        .list_cards(CardListFilter::default())?
        .iter()
        .map(|s| s.id)
        .collect();
    assert!(
        !ids.contains(&a_card),
        "archived board's card hidden from list_cards"
    );
    assert!(ids.contains(&b_card));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archived_board_cards_hidden_from_list_all_cards_and_accessor() -> KanbanResult<()> {
    let mut c = ctx().await;
    let (_a, a_card, b_card) = seed(&mut c)?;
    // Public trait method (live-scoped).
    let all = c.list_all_cards()?;
    assert!(!all.iter().any(|x| x.id == a_card));
    assert!(all.iter().any(|x| x.id == b_card));
    // Inherent accessor (used by the TUI) — also live.
    let acc = c.cards()?;
    assert!(!acc.iter().any(|x| x.id == a_card));
    // Columns + sprints accessors exclude the archived board too.
    assert_eq!(c.columns()?.len(), 1, "only the live board's column");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archived_board_card_not_resolvable_by_identifier() -> KanbanResult<()> {
    let mut c = ctx().await;
    let (_a, a_card, _b) = seed(&mut c)?;
    let found = c.find_cards_by_identifier(&a_card.to_string())?;
    assert!(
        found.is_empty(),
        "an archived board's card must not resolve in a user-facing lookup"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_snapshot_and_export_still_carry_archived_board_subtree() -> KanbanResult<()> {
    let mut c = ctx().await;
    let (_a, a_card, _b) = seed(&mut c)?;
    // FIDELITY: snapshot keeps the archived board's card (no data loss).
    let snap = c.snapshot()?;
    assert!(
        snap.cards.iter().any(|x| x.id == a_card),
        "snapshot must preserve the archived board's card"
    );
    assert_eq!(snap.archived_boards.len(), 1);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_relations_for_board_keeps_internal_edges_drops_cross_board() -> KanbanResult<()>
{
    // C10a: an archived board's relations view must show edges internal to the
    // board and MUST NOT leak a cross-board edge (the global graph is card-keyed
    // with no board dimension, so scoping is this read's job).
    let mut c = ctx().await;
    let a = c.create_board("A".into(), None)?;
    let a_col = c.create_column(a.id, "Todo".into(), None)?;
    let a1 = c.create_card(a.id, a_col.id, "a1".into(), Default::default())?;
    let a2 = c.create_card(a.id, a_col.id, "a2".into(), Default::default())?;
    let b = c.create_board("B".into(), None)?;
    let b_col = c.create_column(b.id, "Todo".into(), None)?;
    let b1 = c.create_card(b.id, b_col.id, "b1".into(), Default::default())?;

    c.attach_children(a1.id, vec![a2.id])?; // internal spawns a1 -> a2
    c.relate(a1.id, b1.id, RelatesKind::default())?; // cross-board relate a1 <-> b1
    c.archive_board(a.id)?;

    let rel = c.list_relations_for_board(a.id)?;
    assert!(
        rel.spawns
            .iter()
            .any(|e| e.base.source == a1.id && e.base.target == a2.id),
        "internal spawns edge is present for the archived board"
    );
    assert!(
        rel.relates.is_empty(),
        "cross-board relate must be excluded (b1 is not in board A)"
    );
    assert!(rel.blocks.is_empty());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_scoped_to_archived_board_returns_its_cards() -> KanbanResult<()> {
    // C10a keystone: an UNSCOPED read hides archived-board cards (C3b), but an
    // EXPLICIT board_id filter is a deliberate scoped request ("show me this
    // board") and must honor the board whether it is live or archived.
    let mut c = ctx().await;
    let (a, a_card, b_card) = seed(&mut c)?;

    let scoped = c.list_cards(CardListFilter {
        board_id: Some(a),
        ..Default::default()
    })?;
    assert!(
        scoped.iter().any(|s| s.id == a_card),
        "scoping to an archived board must return its cards"
    );
    assert!(
        !scoped.iter().any(|s| s.id == b_card),
        "scoping to board A must not leak board B's card"
    );

    // C3b invariant preserved: the UNSCOPED list still hides the archived card.
    let unscoped = c.list_cards(CardListFilter::default())?;
    assert!(!unscoped.iter().any(|s| s.id == a_card));
    assert!(unscoped.iter().any(|s| s.id == b_card));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_restore_board_returns_cards_to_live_views() -> KanbanResult<()> {
    let mut c = ctx().await;
    let (a, a_card, _b) = seed(&mut c)?;
    c.restore_board(a)?;
    let all = c.list_all_cards()?;
    assert!(
        all.iter().any(|x| x.id == a_card),
        "restoring the board returns its cards to live views"
    );
    Ok(())
}
