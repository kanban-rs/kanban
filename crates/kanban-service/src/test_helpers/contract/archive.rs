use super::super::BackendFactory;
use super::assert_card_eq;
use crate::KanbanContext;
use kanban_core::AppConfig;
use kanban_domain::archival::ArchivedEntity;
use kanban_domain::card::CardPriority;
use kanban_domain::{CardListFilter, CreateCardOptions, KanbanOperations};
use tempfile::TempDir;

pub async fn test_archive_card_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), Some("B".into())).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();

    let card = ctx
        .create_card(
            board.id,
            col.id,
            "To Archive".into(),
            CreateCardOptions {
                description: Some("archived desc".into()),
                priority: Some(CardPriority::High),
                points: Some(3),
                ..Default::default()
            },
        )
        .unwrap();
    // The full pre-archive entity — every field must survive the round-trip.
    let pre_archive = ctx.get_card(card.id).unwrap().unwrap();

    ctx.archive_card(card.id).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    // F1 (KAN-870): `get_card` is UNFILTERED — an archived card stays live behind
    // a marker and is reachable by id (it is an ordinary, editable card). It is
    // only hidden from the LIVE list.
    assert!(
        ctx.get_card(card.id).unwrap().is_some(),
        "get_card returns the archived card unfiltered"
    );
    assert!(
        !ctx.list_all_cards()
            .unwrap()
            .iter()
            .any(|c| c.id == card.id),
        "archived card is hidden from the live list"
    );

    let archived = ctx.list_archived_cards().unwrap();
    assert_eq!(archived.len(), 1);

    // F3b marker: only `entity_id` (== card.id) and `context.board_id` are on the
    // marker. The whole entity is asserted via the still-live card fetched by id.
    let ac = &archived[0];
    assert_eq!(ac.entity_id(), card.id);
    assert_eq!(ac.entity_id, card.id);
    assert_eq!(ac.context.board_id, board.id);

    let live = ctx.get_card(ac.entity_id).unwrap().unwrap();
    assert_card_eq(&live, &pre_archive);
}

pub async fn test_archive_card_with_sprint_logs_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), Some("B".into())).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let sprint = ctx.create_sprint(board.id, None, None).unwrap();
    ctx.activate_sprint(sprint.id, Some(14)).unwrap();

    let card = ctx
        .create_card(
            board.id,
            col.id,
            "Sprint Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.assign_card_to_sprint(card.id, sprint.id).unwrap();
    ctx.archive_card(card.id).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let archived = ctx.list_archived_cards().unwrap();
    assert_eq!(archived.len(), 1);
    let live = ctx.get_card(archived[0].entity_id).unwrap().unwrap();
    assert!(!live.sprint_logs.is_empty());
}

pub async fn test_restore_archived_card_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), Some("B".into())).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();

    let card = ctx
        .create_card(
            board.id,
            col.id,
            "Will Restore".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let pre_archive = ctx.get_card(card.id).unwrap().unwrap();

    ctx.archive_card(card.id).unwrap();
    ctx.restore_card(card.id, None).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    // The card stays live in place across archive→restore; every field survives
    // EXCEPT `updated_at`, which `RestoreCard` re-stamps to the restore time.
    let c = ctx.get_card(card.id).unwrap().unwrap();
    let expected = kanban_domain::Card {
        updated_at: c.updated_at,
        ..pre_archive
    };
    assert_card_eq(&c, &expected);
    assert!(ctx.list_archived_cards().unwrap().is_empty());
}

/// An archived card is an ordinary LIVE card behind a marker; editing it (here:
/// clearing its sprint by deleting the sprint, which drives
/// `clear_sprint_from_archived_cards`) must reach the live card AND survive a
/// save/reload — on every backend. Every OTHER field must be untouched.
pub async fn test_edit_archived_card_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), Some("B".into())).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let sprint = ctx.create_sprint(board.id, None, None).unwrap();

    let card = ctx
        .create_card(
            board.id,
            col.id,
            "Edited While Archived".into(),
            CreateCardOptions {
                description: Some("body".into()),
                priority: Some(CardPriority::High),
                points: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
    ctx.assign_card_to_sprint(card.id, sprint.id).unwrap();
    ctx.archive_card(card.id).unwrap();

    // Snapshot the archived-but-live card BEFORE the edit (it carries the sprint).
    let pre_edit = ctx.get_card(card.id).unwrap().unwrap();
    assert_eq!(
        pre_edit.sprint_id,
        Some(sprint.id),
        "sprint assigned pre-edit"
    );

    // Edit the archived card in place: deleting its sprint clears sprint_id on the
    // LIVE archived card via `clear_sprint_from_archived_cards`.
    ctx.delete_sprint(sprint.id).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    // Still archived (hidden from live, present as a marker).
    let archived = ctx.list_archived_cards().unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].entity_id, card.id);

    // The edit reached the live card and survived the round-trip.
    let live = ctx.get_card(card.id).unwrap().unwrap();
    assert_eq!(
        live.sprint_id, None,
        "the sprint clear reached the live archived card and persisted"
    );

    // Every OTHER field is unchanged from the pre-edit snapshot (only sprint_id
    // and updated_at may move; build the expectation by clearing the sprint on the
    // pre-edit copy and letting updated_at be the observed value).
    let expected = kanban_domain::Card {
        sprint_id: None,
        updated_at: live.updated_at,
        ..pre_edit
    };
    assert_card_eq(&live, &expected);
}

/// F7 (KAN-889): the unified `list_cards` is the ONE path for live/archived/both
/// via `CardListFilter::archived`. Seed one live + one archived card and assert
/// each selector state returns exactly the right set, each `CardSummary` stamped
/// with the correct `archived_at`. Also guards that the DEFAULT filter (LiveOnly)
/// is unchanged.
pub async fn test_list_cards_archived_selector_roundtrip(factory: &BackendFactory) {
    use kanban_domain::ArchivedFilter;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), Some("B".into())).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let live = ctx
        .create_card(
            board.id,
            col.id,
            "Live".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let archived = ctx
        .create_card(
            board.id,
            col.id,
            "Archived".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.archive_card(archived.id).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    // The DEFAULT filter is LiveOnly and MUST be unchanged: only the live card,
    // with no archived_at.
    let default_ids: Vec<_> = ctx
        .list_cards(CardListFilter::default())
        .unwrap()
        .into_iter()
        .map(|s| (s.id, s.archived_at))
        .collect();
    assert_eq!(default_ids, vec![(live.id, None)], "default == LiveOnly");

    // LiveOnly (explicit): same as default.
    let live_only = ctx
        .list_cards(CardListFilter {
            archived: ArchivedFilter::LiveOnly,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(live_only.len(), 1);
    assert_eq!(live_only[0].id, live.id);
    assert_eq!(live_only[0].archived_at, None);

    // ArchivedOnly: exactly the archived card, stamped with archived_at.
    let archived_only = ctx
        .list_cards(CardListFilter {
            archived: ArchivedFilter::ArchivedOnly,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(archived_only.len(), 1);
    assert_eq!(archived_only[0].id, archived.id);
    assert!(
        archived_only[0].archived_at.is_some(),
        "archived summary carries archived_at"
    );

    // Include: both, no duplicates, each stamped correctly.
    let include = ctx
        .list_cards(CardListFilter {
            archived: ArchivedFilter::Include,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(include.len(), 2, "union of live + archived, no duplicates");
    let live_summary = include.iter().find(|s| s.id == live.id).unwrap();
    let arch_summary = include.iter().find(|s| s.id == archived.id).unwrap();
    assert_eq!(live_summary.archived_at, None, "live stays None");
    assert!(arch_summary.archived_at.is_some(), "archived stays Some");
}

/// The same three-state selector, but board-scoped (an explicit `board_id`).
pub async fn test_list_cards_archived_selector_board_scoped(factory: &BackendFactory) {
    use kanban_domain::ArchivedFilter;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), Some("B".into())).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();
    // A second board with its own cards, to prove board scoping holds across the
    // selector (its cards never leak into the target board's results).
    let other = ctx.create_board("Other".into(), Some("O".into())).unwrap();
    let other_col = ctx.create_column(other.id, "OC".into(), None).unwrap();
    let other_live = ctx
        .create_card(
            other.id,
            other_col.id,
            "OL".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let other_arch = ctx
        .create_card(
            other.id,
            other_col.id,
            "OA".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.archive_card(other_arch.id).unwrap();

    let live = ctx
        .create_card(
            board.id,
            col.id,
            "Live".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let archived = ctx
        .create_card(
            board.id,
            col.id,
            "Archived".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.archive_card(archived.id).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let scoped = |archived: ArchivedFilter| CardListFilter {
        board_id: Some(board.id),
        archived,
        ..Default::default()
    };

    // LiveOnly: only the target board's live card.
    let live_only = ctx.list_cards(scoped(ArchivedFilter::LiveOnly)).unwrap();
    assert_eq!(live_only.len(), 1);
    assert_eq!(live_only[0].id, live.id);
    assert!(!live_only.iter().any(|s| s.id == other_live.id));

    // ArchivedOnly: only the target board's archived card, stamped.
    let archived_only = ctx
        .list_cards(scoped(ArchivedFilter::ArchivedOnly))
        .unwrap();
    assert_eq!(archived_only.len(), 1);
    assert_eq!(archived_only[0].id, archived.id);
    assert!(archived_only[0].archived_at.is_some());
    assert!(!archived_only.iter().any(|s| s.id == other_arch.id));

    // Include: both of the target board's cards; nothing from the other board.
    let include = ctx.list_cards(scoped(ArchivedFilter::Include)).unwrap();
    assert_eq!(include.len(), 2);
    assert!(include.iter().any(|s| s.id == live.id));
    assert!(include.iter().any(|s| s.id == archived.id));
    assert!(!include
        .iter()
        .any(|s| s.id == other_live.id || s.id == other_arch.id));
}
