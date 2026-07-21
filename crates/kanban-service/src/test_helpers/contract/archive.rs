use super::super::BackendFactory;
use super::assert_card_eq;
use crate::KanbanContext;
use kanban_core::AppConfig;
use kanban_domain::archival::ArchivedEntity;
use kanban_domain::card::CardPriority;
use kanban_domain::{
    CardListFilter, CreateCardOptions, GraphOperations, KanbanOperations, Severity,
};
use std::collections::HashSet;
use tempfile::TempDir;
use uuid::Uuid;

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

/// KAN-898: `clear_sprint_from_cards` must skip archived cards on every backend
/// (live-only semantics), matching SQLite's `AND NOT EXISTS archived_cards` guard.
/// Calling it on a sprint that is assigned to both a live and an archived card must
/// clear the live card's `sprint_id` but leave the archived card's `sprint_id`
/// (and `updated_at`) untouched. Save + reload verifies the split holds through
/// persistence.
pub async fn test_clear_sprint_from_cards_leaves_archived_untouched(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), Some("B".into())).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let sprint = ctx.create_sprint(board.id, None, None).unwrap();

    let live_card = ctx
        .create_card(
            board.id,
            col.id,
            "Live".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let archived_card = ctx
        .create_card(
            board.id,
            col.id,
            "Archived".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    ctx.assign_card_to_sprint(live_card.id, sprint.id).unwrap();
    ctx.assign_card_to_sprint(archived_card.id, sprint.id)
        .unwrap();

    // Capture archived card's sprint_id and updated_at BEFORE archiving
    // (archiving may bump updated_at, but clear_sprint_from_cards must not touch it).
    ctx.archive_card(archived_card.id).unwrap();
    let archived_before = ctx.get_card(archived_card.id).unwrap().unwrap();
    assert_eq!(
        archived_before.sprint_id,
        Some(sprint.id),
        "archived card has sprint pre-clear"
    );

    let ts = chrono::Utc::now();
    ctx.data_store()
        .clear_sprint_from_cards(sprint.id, ts)
        .unwrap();

    // Live card: sprint_id must be cleared.
    let live_after = ctx.get_card(live_card.id).unwrap().unwrap();
    assert_eq!(
        live_after.sprint_id, None,
        "live card sprint cleared by clear_sprint_from_cards"
    );

    // Archived card: sprint_id must NOT be touched.
    let archived_after = ctx.get_card(archived_card.id).unwrap().unwrap();
    assert_eq!(
        archived_after.sprint_id,
        Some(sprint.id),
        "archived card sprint_id must be untouched by clear_sprint_from_cards"
    );
    assert_eq!(
        archived_after.updated_at, archived_before.updated_at,
        "archived card updated_at must not be bumped"
    );

    // Save + reload: the split must survive persistence.
    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let live_reloaded = ctx.get_card(live_card.id).unwrap().unwrap();
    assert_eq!(
        live_reloaded.sprint_id, None,
        "live card sprint_id still None after reload"
    );
    let archived_reloaded = ctx.get_card(archived_card.id).unwrap().unwrap();
    assert_eq!(
        archived_reloaded.sprint_id,
        Some(sprint.id),
        "archived card sprint_id still Some after reload"
    );
}

/// KAN-899: `delete_board` (the bare primitive) must be a no-op on an archived
/// board on every backend, matching SQLite's `AND NOT EXISTS board_archival` guard.
/// Calling it on an archived board must leave the head and its subtree intact.
pub async fn test_delete_board_is_noop_on_archived_board(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let _ = ctx
        .create_card(
            board.id,
            col.id,
            "Task".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    ctx.archive_board(board.id).unwrap();

    // Call the raw DataStore primitive directly.
    ctx.data_store().delete_board(board.id).unwrap();

    // Board head must still be fetchable (get_board is unfiltered).
    assert!(
        ctx.data_store().get_board(board.id).unwrap().is_some(),
        "archived board head must survive bare delete_board"
    );
    // Archived marker must still be present.
    assert!(
        ctx.data_store()
            .get_archived_board(board.id)
            .unwrap()
            .is_some(),
        "archived marker must still be present after bare delete_board"
    );
    // Subtree must be intact.
    let snap = ctx.data_store().snapshot().unwrap();
    assert_eq!(
        snap.columns.len(),
        1,
        "column must survive bare delete_board on archived board"
    );
    assert_eq!(
        snap.cards.len(),
        1,
        "card must survive bare delete_board on archived board"
    );
}

/// KAN-908: rich-seed struct for board delete/undo and archive/restore full-graph
/// round-trip tests.
struct RichSeed {
    board: Uuid,
    live: Uuid,
    arch: Uuid,
    sprint: Uuid,
}

fn seed_rich(ctx: &mut KanbanContext) -> kanban_domain::KanbanResult<RichSeed> {
    let b = ctx.create_board("Proj".into(), None)?;
    let col = ctx.create_column(b.id, "Todo".into(), None)?;
    let sprint = ctx.create_sprint(b.id, None, None)?;
    let live = ctx.create_card(b.id, col.id, "Live".into(), CreateCardOptions::default())?;
    let arch = ctx.create_card(b.id, col.id, "Arch".into(), CreateCardOptions::default())?;
    ctx.assign_card_to_sprint(live.id, sprint.id)?;
    ctx.block(live.id, arch.id, Severity::High)?;
    ctx.archive_card(arch.id)?;
    Ok(RichSeed {
        board: b.id,
        live: live.id,
        arch: arch.id,
        sprint: sprint.id,
    })
}

/// KAN-908: board delete+undo must be the identity over the FULL entity graph.
/// Seeds ≥1 column + live card + inner individually-archived card + sprint +
/// dependency edge, archives + permanently deletes the board, asserts the whole
/// graph is gone, undoes, then asserts EVERY owned/referenced entity type is back.
/// Save+reload before the final asserts on persistent backends.
pub async fn test_board_delete_undo_full_graph_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let s = seed_rich(&mut ctx).unwrap();

    ctx.archive_board(s.board).unwrap();
    ctx.delete_board(s.board).unwrap();

    // Everything gone after permanent delete.
    let empty = ctx.data_store().snapshot().unwrap();
    assert!(empty.cards.is_empty(), "all card rows gone after delete");
    assert!(empty.columns.is_empty(), "columns gone after delete");
    assert!(empty.sprints.is_empty(), "sprints gone after delete");
    assert!(
        empty.archived_cards.is_empty(),
        "inner archived-card marker gone after delete"
    );
    assert_eq!(empty.graph.len(), 0, "dependency edge gone after delete");

    assert!(ctx.undo().unwrap(), "undo returned true");

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let snap = ctx.data_store().snapshot().unwrap();
    assert_eq!(snap.archived_boards.len(), 1, "board restored as archived");
    assert_eq!(snap.columns.len(), 1, "column restored after undo");
    assert_eq!(snap.cards.len(), 2, "both card rows restored after undo");
    assert_eq!(snap.sprints.len(), 1, "sprint restored after undo");
    assert_eq!(
        snap.archived_cards.len(),
        1,
        "inner archived-card marker restored after undo"
    );
    assert_eq!(snap.graph.len(), 1, "dependency edge restored after undo");
    assert!(
        ctx.get_card(s.live).unwrap().is_some(),
        "live card reachable after undo"
    );
    assert!(
        ctx.get_card(s.arch).unwrap().is_some(),
        "archived card reachable after undo"
    );
    assert_eq!(
        ctx.get_card(s.live).unwrap().unwrap().sprint_id,
        Some(s.sprint),
        "sprint binding restored after undo"
    );
    assert!(
        !ctx.get_card(s.live)
            .unwrap()
            .unwrap()
            .sprint_logs
            .is_empty(),
        "sprint_logs survived undo"
    );
}

/// KAN-938: the single-board export path (`export_board(Some(id))`, the CLI/MCP
/// V2 path) must carry the board's archived cards in the exported snapshot, at
/// parity with the full/all-boards export path. Seed a board with a live card
/// and an archived card, export just that board, and assert the archived card's
/// marker is present in the exported snapshot on every backend.
pub async fn test_single_board_export_includes_archived_cards(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), Some("B".into())).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let _live = ctx
        .create_card(board.id, col.id, "Live".into(), CreateCardOptions::default())
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

    let json = ctx.export_board(Some(board.id)).unwrap();
    let snapshot: kanban_domain::Snapshot = serde_json::from_str(&json).unwrap();

    assert_eq!(
        snapshot.archived_cards.len(),
        1,
        "single-board export must carry the board's archived-card marker"
    );
    assert_eq!(
        snapshot.archived_cards[0].entity_id(),
        archived.id,
        "exported archived-card marker references the archived card"
    );
    assert_eq!(
        snapshot.archived_cards[0].context.board_id, board.id,
        "exported archived-card marker carries its board_id"
    );
    // The live row of the archived card must still be present so the marker is
    // not orphaned on import.
    assert!(
        snapshot.cards.iter().any(|c| c.id == archived.id),
        "live row of the archived card is carried in the export"
    );
}

/// KAN-938: a single-board export→import round-trip must preserve the board's
/// archived card — it stays archived and scoped to the same board — on every
/// backend. This is the reversibility invariant for the CLI/MCP V2 export path.
pub async fn test_single_board_export_roundtrips_archived_card(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let src_path = dir.path().join("src.store");
    let mut src = KanbanContext::open(factory(&src_path), AppConfig::default())
        .await
        .unwrap();

    let board = src.create_board("Board".into(), Some("B".into())).unwrap();
    let col = src.create_column(board.id, "Col".into(), None).unwrap();
    let _live = src
        .create_card(board.id, col.id, "Live".into(), CreateCardOptions::default())
        .unwrap();
    let archived = src
        .create_card(
            board.id,
            col.id,
            "Archived".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    src.archive_card(archived.id).unwrap();

    let json = src.export_board(Some(board.id)).unwrap();

    let dst_path = dir.path().join("dst.store");
    let mut dst = KanbanContext::open(factory(&dst_path), AppConfig::default())
        .await
        .unwrap();
    dst.import_board(&json).unwrap();

    dst.save().await.unwrap();
    let dst = KanbanContext::open_deferred(factory(&dst_path), AppConfig::default());

    let imported_archived = dst.list_archived_cards().unwrap();
    assert_eq!(
        imported_archived.len(),
        1,
        "imported single-board export keeps the archived card archived"
    );
    assert_eq!(
        imported_archived[0].entity_id(),
        archived.id,
        "imported archived card is the same card"
    );
    assert_eq!(
        imported_archived[0].context.board_id, board.id,
        "imported archived card stays scoped to its board"
    );
    // It is hidden from the live list but reachable by id (F1 marker model).
    assert!(
        !dst.list_all_cards()
            .unwrap()
            .iter()
            .any(|c| c.id == archived.id),
        "restored archived card is hidden from the live list"
    );
    assert!(
        dst.get_card(archived.id).unwrap().is_some(),
        "archived card is reachable by id after round-trip"
    );
}

/// KAN-938: a board that is ITSELF archived must carry its `ArchivedBoard` marker
/// through a single-board export→import round-trip on every backend. The board
/// comes back as archived (a marker present, hidden from the live list).
pub async fn test_single_board_export_roundtrips_archived_board_marker(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let src_path = dir.path().join("src.store");
    let mut src = KanbanContext::open(factory(&src_path), AppConfig::default())
        .await
        .unwrap();

    let board = src.create_board("Board".into(), Some("B".into())).unwrap();
    let col = src.create_column(board.id, "Col".into(), None).unwrap();
    let _card = src
        .create_card(board.id, col.id, "Task".into(), CreateCardOptions::default())
        .unwrap();
    src.archive_board(board.id).unwrap();

    let json = src.export_board(Some(board.id)).unwrap();
    let snapshot: kanban_domain::Snapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(
        snapshot.archived_boards.len(),
        1,
        "single-board export of an archived board carries its ArchivedBoard marker"
    );
    assert_eq!(
        snapshot.archived_boards[0].entity_id(),
        board.id,
        "exported archived-board marker references the board"
    );

    let dst_path = dir.path().join("dst.store");
    let mut dst = KanbanContext::open(factory(&dst_path), AppConfig::default())
        .await
        .unwrap();
    dst.import_board(&json).unwrap();

    dst.save().await.unwrap();
    let dst = KanbanContext::open_deferred(factory(&dst_path), AppConfig::default());

    let imported = dst.data_store().snapshot().unwrap();
    assert_eq!(
        imported.archived_boards.len(),
        1,
        "imported board stays archived (marker survives the round-trip)"
    );
    assert_eq!(imported.archived_boards[0].entity_id(), board.id);
    assert!(
        !dst.list_boards().unwrap().iter().any(|b| b.id == board.id),
        "archived board is hidden from the live board list after round-trip"
    );
    assert!(
        dst.data_store().get_board(board.id).unwrap().is_some(),
        "archived board head is reachable after round-trip"
    );
}

/// KAN-901: archived card with a deleted column is still returned by
/// list_cards(ArchivedOnly) — the selector path scopes by marker board_id,
/// not by current column membership.
pub async fn test_list_cards_archived_only_keeps_card_with_deleted_column(
    factory: &BackendFactory,
) {
    use kanban_domain::ArchivedFilter;

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
            "Archived".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.archive_card(card.id).unwrap();
    // Delete the column AFTER archival — card's live row still references it.
    ctx.delete_column(col.id).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let archived = ctx
        .list_cards(CardListFilter {
            board_id: Some(board.id),
            archived: ArchivedFilter::ArchivedOnly,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        archived.len(),
        1,
        "archived card with deleted original column must still appear in ArchivedOnly list"
    );
    assert_eq!(archived[0].id, card.id);
}

/// KAN-901: board default sort is honoured by list_cards(ArchivedOnly).
pub async fn test_list_cards_archived_only_board_default_sort(factory: &BackendFactory) {
    use kanban_domain::{ArchivedFilter, BoardUpdate, CardPriority, SortField, SortOrder};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), Some("B".into())).unwrap();
    // Set board default sort to priority descending.
    ctx.update_board(
        board.id,
        BoardUpdate {
            task_sort_field: Some(SortField::Priority),
            task_sort_order: Some(SortOrder::Descending),
            ..Default::default()
        },
    )
    .unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let low = ctx
        .create_card(
            board.id,
            col.id,
            "Low".into(),
            CreateCardOptions {
                priority: Some(CardPriority::Low),
                ..Default::default()
            },
        )
        .unwrap();
    let high = ctx
        .create_card(
            board.id,
            col.id,
            "High".into(),
            CreateCardOptions {
                priority: Some(CardPriority::High),
                ..Default::default()
            },
        )
        .unwrap();
    ctx.archive_card(low.id).unwrap();
    ctx.archive_card(high.id).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let archived = ctx
        .list_cards(CardListFilter {
            board_id: Some(board.id),
            archived: ArchivedFilter::ArchivedOnly,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(archived.len(), 2);
    // Priority descending: High first.
    assert_eq!(
        archived[0].id, high.id,
        "board default sort (priority desc) must put High-priority card first"
    );
    assert_eq!(archived[1].id, low.id);
}

/// KAN-901: explicit sort override wins over board default for ArchivedOnly.
pub async fn test_list_cards_archived_only_explicit_override_wins(factory: &BackendFactory) {
    use kanban_domain::{ArchivedFilter, CardPriority, SortField, SortOrder};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), Some("B".into())).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let low = ctx
        .create_card(
            board.id,
            col.id,
            "Low".into(),
            CreateCardOptions {
                priority: Some(CardPriority::Low),
                ..Default::default()
            },
        )
        .unwrap();
    let high = ctx
        .create_card(
            board.id,
            col.id,
            "High".into(),
            CreateCardOptions {
                priority: Some(CardPriority::High),
                ..Default::default()
            },
        )
        .unwrap();
    ctx.archive_card(low.id).unwrap();
    ctx.archive_card(high.id).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    // Explicit sort override: priority ascending — Low first.
    let archived = ctx
        .list_cards(CardListFilter {
            board_id: Some(board.id),
            sort: Some(SortField::Priority),
            sort_order: Some(SortOrder::Ascending),
            archived: ArchivedFilter::ArchivedOnly,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(archived.len(), 2);
    assert_eq!(
        archived[0].id, low.id,
        "explicit sort override (priority asc) must put Low-priority card first"
    );
    assert_eq!(archived[1].id, high.id);
}

/// KAN-908: board archive+restore must be the identity over the FULL entity
/// graph. Seeds the same rich graph, archives, restores, save+reload, then
/// asserts every entity type survived unchanged.
pub async fn test_board_archive_restore_full_graph_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let s = seed_rich(&mut ctx).unwrap();

    ctx.archive_board(s.board).unwrap();
    ctx.restore_board(s.board).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let snap = ctx.data_store().snapshot().unwrap();
    assert_eq!(snap.boards.len(), 1, "board is live after restore");
    assert!(snap.archived_boards.is_empty(), "no archived-board marker");
    assert_eq!(snap.columns.len(), 1, "column survived archive/restore");
    assert_eq!(
        snap.cards.len(),
        2,
        "both card rows survived archive/restore"
    );
    assert_eq!(snap.sprints.len(), 1, "sprint survived archive/restore");
    assert_eq!(
        snap.archived_cards.len(),
        1,
        "inner archived-card marker survived archive/restore"
    );
    assert_eq!(
        snap.graph.len(),
        1,
        "dependency edge survived archive/restore"
    );
    assert!(
        ctx.get_card(s.live).unwrap().is_some(),
        "live card reachable after restore"
    );
    assert!(
        ctx.get_card(s.arch).unwrap().is_some(),
        "archived card reachable after restore"
    );
    assert_eq!(
        ctx.get_card(s.live).unwrap().unwrap().sprint_id,
        Some(s.sprint),
        "sprint binding survived archive/restore"
    );
    assert!(
        !ctx.get_card(s.live)
            .unwrap()
            .unwrap()
            .sprint_logs
            .is_empty(),
        "sprint_logs survived archive/restore"
    );
}

/// B2 (KAN-918): `list_boards_filtered` is the ONE path for live/archived/both
/// board heads via `BoardListFilter::archived`, mirroring `list_cards`. Seed one
/// live + one archived board and assert each selector state returns exactly the
/// right set. Held to one spec across all backends via the contract macro.
pub async fn test_list_boards_archived_selector_roundtrip(factory: &BackendFactory) {
    use kanban_domain::{ArchivedFilter, BoardListFilter};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let live = ctx.create_board("Live".into(), Some("L".into())).unwrap();
    let archived = ctx
        .create_board("Archived".into(), Some("A".into()))
        .unwrap();
    ctx.archive_board(archived.id).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    // LiveOnly: exactly the live board, byte-identical to the legacy list_boards.
    let live_only = ctx
        .list_boards_filtered(BoardListFilter {
            archived: ArchivedFilter::LiveOnly,
            ..Default::default()
        })
        .unwrap();
    let live_only_ids: Vec<_> = live_only.iter().map(|b| b.id).collect();
    assert_eq!(live_only_ids, vec![live.id], "LiveOnly excludes archived");
    assert_eq!(
        ctx.list_boards()
            .unwrap()
            .iter()
            .map(|b| b.id)
            .collect::<Vec<_>>(),
        live_only_ids,
        "list_boards() == LiveOnly sugar",
    );

    // ArchivedOnly: exactly the archived board head.
    let archived_only = ctx
        .list_boards_filtered(BoardListFilter {
            archived: ArchivedFilter::ArchivedOnly,
            ..Default::default()
        })
        .unwrap();
    let archived_only_ids: Vec<_> = archived_only.iter().map(|b| b.id).collect();
    assert_eq!(
        archived_only_ids,
        vec![archived.id],
        "ArchivedOnly returns only archived",
    );

    // Include: both heads, no duplicates.
    let include = ctx
        .list_boards_filtered(BoardListFilter {
            archived: ArchivedFilter::Include,
            ..Default::default()
        })
        .unwrap();
    let include_ids: HashSet<_> = include.iter().map(|b| b.id).collect();
    assert_eq!(
        include.len(),
        2,
        "Include unions live + archived, no duplicates"
    );
    assert_eq!(
        include_ids,
        HashSet::from([live.id, archived.id]),
        "Include returns both heads",
    );
}

/// S4 (KAN-945): a request-level `sort` on `BoardListFilter` sorts the returned
/// heads server-side. Seed boards out of alphabetical order and assert
/// `sort = Name` (ascending default) returns them alphabetically, on every
/// backend.
pub async fn test_list_boards_filtered_sorts_by_request_sort(factory: &BackendFactory) {
    use kanban_domain::{BoardListFilter, BoardSortField};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    ctx.create_board("Charlie".into(), None).unwrap();
    ctx.create_board("Alpha".into(), None).unwrap();
    ctx.create_board("Bravo".into(), None).unwrap();

    let sorted = ctx
        .list_boards_filtered(BoardListFilter {
            sort: Some(BoardSortField::Name),
            ..Default::default()
        })
        .unwrap();
    let names: Vec<_> = sorted.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["Alpha", "Bravo", "Charlie"],
        "request sort=Name orders boards alphabetically"
    );
}

/// S4 (KAN-945): a request-level `sort_order = Descending` reverses the sort.
pub async fn test_list_boards_filtered_order_desc_reverses(factory: &BackendFactory) {
    use kanban_domain::{BoardListFilter, BoardSortField, SortOrder};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    ctx.create_board("Alpha".into(), None).unwrap();
    ctx.create_board("Charlie".into(), None).unwrap();
    ctx.create_board("Bravo".into(), None).unwrap();

    let sorted = ctx
        .list_boards_filtered(BoardListFilter {
            sort: Some(BoardSortField::Name),
            sort_order: Some(SortOrder::Descending),
            ..Default::default()
        })
        .unwrap();
    let names: Vec<_> = sorted.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["Charlie", "Bravo", "Alpha"],
        "request sort_order=Descending reverses the board order"
    );
}

/// S4 (KAN-945): with NO request sort, the service falls back to the AppConfig
/// default (`board_sort_field`/`board_sort_order`). Configure `Name` and assert
/// the boards come back alphabetically without any request-level sort.
pub async fn test_list_boards_filtered_falls_back_to_config_default(factory: &BackendFactory) {
    use kanban_domain::BoardListFilter;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let config = AppConfig {
        board_sort_field: Some("Name".into()),
        board_sort_order: Some("Ascending".into()),
        ..Default::default()
    };
    let mut ctx = KanbanContext::open(factory(&path), config).await.unwrap();

    ctx.create_board("Charlie".into(), None).unwrap();
    ctx.create_board("Alpha".into(), None).unwrap();
    ctx.create_board("Bravo".into(), None).unwrap();

    // No request-level sort — the AppConfig default drives the order.
    let sorted = ctx
        .list_boards_filtered(BoardListFilter::default())
        .unwrap();
    let names: Vec<_> = sorted.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["Alpha", "Bravo", "Charlie"],
        "AppConfig board_sort_field=Name drives the order when no request sort is set"
    );
}

/// S4 (KAN-945): with NO config and NO request sort, the live board list stays
/// in Position order — byte-identical to `list_boards()`. This is the guard that
/// the default configuration does not perturb today's ordering.
pub async fn test_list_boards_no_config_no_request_is_position_order(factory: &BackendFactory) {
    use kanban_domain::BoardListFilter;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    // Created in this order → positions 0,1,2. Names are NOT alphabetical, so a
    // stray Name sort would reorder them and fail this guard.
    ctx.create_board("Charlie".into(), None).unwrap();
    ctx.create_board("Alpha".into(), None).unwrap();
    ctx.create_board("Bravo".into(), None).unwrap();

    let filtered_ids: Vec<_> = ctx
        .list_boards_filtered(BoardListFilter::default())
        .unwrap()
        .iter()
        .map(|b| b.id)
        .collect();
    let legacy_ids: Vec<_> = ctx.list_boards().unwrap().iter().map(|b| b.id).collect();
    assert_eq!(
        filtered_ids, legacy_ids,
        "unconfigured list_boards_filtered stays position-ordered, byte-identical to list_boards()"
    );
}

/// R3 (KAN-952): with NO config and NO request sort, the built-in default is
/// chosen PER CONTEXT from the filter's archived selector. `ArchivedOnly` picks
/// `DEFAULT_ARCHIVED_BOARD_SORT` (most-recently-archived first), so archiving A
/// then B returns [B, A] on every backend.
pub async fn test_list_boards_archived_only_default_is_recency(factory: &BackendFactory) {
    use kanban_domain::{ArchivedFilter, BoardListFilter};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    // Archive A first, then B → B is the most recently archived.
    let a = ctx.create_board("Alpha".into(), None).unwrap();
    let b = ctx.create_board("Bravo".into(), None).unwrap();
    ctx.archive_board(a.id).unwrap();
    ctx.archive_board(b.id).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let ids: Vec<_> = ctx
        .list_boards_filtered(BoardListFilter {
            archived: ArchivedFilter::ArchivedOnly,
            ..Default::default()
        })
        .unwrap()
        .iter()
        .map(|bd| bd.id)
        .collect();
    assert_eq!(
        ids,
        vec![b.id, a.id],
        "ArchivedOnly default is recency (most recently archived first)"
    );
}

/// R3 (KAN-952): the LiveOnly default stays Position ascending
/// (`DEFAULT_BOARD_SORT_LIVE`), byte-identical to `list_boards()`, even though
/// the ArchivedOnly context now defaults to recency. Names are NOT alphabetical
/// so a stray Name/recency default would reorder and fail.
pub async fn test_list_boards_live_default_is_position(factory: &BackendFactory) {
    use kanban_domain::BoardListFilter;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    ctx.create_board("Charlie".into(), None).unwrap();
    ctx.create_board("Alpha".into(), None).unwrap();
    ctx.create_board("Bravo".into(), None).unwrap();

    let filtered_ids: Vec<_> = ctx
        .list_boards_filtered(BoardListFilter::default())
        .unwrap()
        .iter()
        .map(|b| b.id)
        .collect();
    let legacy_ids: Vec<_> = ctx.list_boards().unwrap().iter().map(|b| b.id).collect();
    assert_eq!(
        filtered_ids, legacy_ids,
        "LiveOnly default stays Position ascending, byte-identical to list_boards()"
    );
}

/// R3 (KAN-952) amplification guard: a plain LiveOnly, non-ArchivedAt request
/// must not perturb its result by (lazily) skipping the archived-marker fetch.
/// We cannot spy the backend call count through the trait, so we assert the
/// observable invariant: the LiveOnly result is exactly `list_boards()` and is
/// unaffected by the presence of archived boards (whose markers are NOT needed).
pub async fn test_list_boards_liveonly_does_not_fetch_archived_markers(factory: &BackendFactory) {
    use kanban_domain::{ArchivedFilter, BoardListFilter};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let live = ctx.create_board("Live".into(), None).unwrap();
    let archived = ctx.create_board("Archived".into(), None).unwrap();
    ctx.archive_board(archived.id).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let live_only: Vec<_> = ctx
        .list_boards_filtered(BoardListFilter {
            archived: ArchivedFilter::LiveOnly,
            ..Default::default()
        })
        .unwrap()
        .iter()
        .map(|b| b.id)
        .collect();
    assert_eq!(
        live_only,
        vec![live.id],
        "LiveOnly returns only the live head, unaffected by archived markers"
    );
    assert_eq!(
        ctx.list_boards()
            .unwrap()
            .iter()
            .map(|b| b.id)
            .collect::<Vec<_>>(),
        live_only,
        "LiveOnly == list_boards() even with archived boards present (no marker fetch needed)"
    );
}
