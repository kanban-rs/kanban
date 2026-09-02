//! KAN-895 regression: the TUI File→Export-All path must fully round-trip an
//! archived board — its HEAD, its entire subtree (columns / cards / sprints,
//! which live in the flat collections under the archived board_id), AND its
//! `archived_boards` marker (so it re-imports still archived, hidden from the
//! live list).
//!
//! Before the fix, `export_all_boards_with_filename` / `auto_save` read the
//! live-scoped `model.boards_state().loaded_or_empty()` / `model.cards_state().loaded_or_empty()`, so an archived board's head
//! and subtree were omitted and the exported `archived_boards` marker referenced
//! a board absent from the file → orphan / silent data loss on re-import. These
//! tests drive the ACTUAL TUI export entry point (not the snapshot path
//! directly).

mod helpers;

use helpers::{FailingSnapshotBackend, SnapshotCountingBackend};
use kanban_domain::{GraphOperations, KanbanOperations};
use kanban_tui::App;
use std::sync::atomic::Ordering;
use tempfile::tempdir;

#[test]
fn test_tui_export_all_round_trips_archived_board_with_subtree() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("export_all_archived.json");

    let mut app = App::test_default();

    // A live board so the export is non-trivial and mixed.
    let live_board = app
        .ctx
        .create_board("Live Board".to_string(), None)
        .unwrap();
    let live_col = app
        .ctx
        .create_column(live_board.id, "Todo".to_string(), None)
        .unwrap();
    app.ctx
        .create_card(
            live_board.id,
            live_col.id,
            "Live Card".to_string(),
            Default::default(),
        )
        .unwrap();

    // The archived board WITH a full subtree: column, card, sprint.
    let arch_board = app
        .ctx
        .create_board("Archived Board".to_string(), None)
        .unwrap();
    let arch_col = app
        .ctx
        .create_column(arch_board.id, "Backlog".to_string(), None)
        .unwrap();
    let arch_card = app
        .ctx
        .create_card(
            arch_board.id,
            arch_col.id,
            "Archived Board Card".to_string(),
            Default::default(),
        )
        .unwrap();
    let arch_sprint = app
        .ctx
        .create_sprint(arch_board.id, Some("S".to_string()), None)
        .unwrap();
    app.ctx.archive_board(arch_board.id).unwrap();
    app.reload_model();
    app.prepare_frame();

    // Sanity: the archived board head is hidden from the live list, present in
    // the archived-boards view.
    assert!(
        app.model
            .live_boards_state()
            .loaded()
            .unwrap()
            .iter()
            .all(|b| b.id != arch_board.id),
        "archived board must be hidden from live list before export"
    );
    assert!(
        app.model.archived_board_ids().contains(&arch_board.id),
        "archived board head must be in the archived set before export"
    );

    // Drive the ACTUAL TUI File→Export-All entry point.
    app.input.set(file_path.to_str().unwrap().to_string());
    app.reload_model();
    app.prepare_frame();
    app.export_all_boards_with_filename().unwrap();

    // Re-import into a FRESH app and assert the whole archived-board graph is back.
    let mut app2 = App::test_default();
    app2.import_board_from_file(file_path.to_str().unwrap())
        .unwrap();
    app2.reload_model();
    app2.prepare_frame();

    // Live board still live.
    assert!(
        app2.model
            .boards_state()
            .loaded_or_empty()
            .iter()
            .any(|b| b.id == live_board.id),
        "live board must be present after re-import"
    );

    // Archived board head is back AND still archived (hidden from live list,
    // present in the archived view with its marker).
    assert!(
        app2.model
            .live_boards_state()
            .loaded()
            .unwrap()
            .iter()
            .all(|b| b.id != arch_board.id),
        "archived board must remain hidden from live list after re-import"
    );
    assert!(
        app2.model
            .boards_state()
            .loaded_or_empty()
            .iter()
            .any(|b| b.id == arch_board.id)
            && app2.model.archived_board_ids().contains(&arch_board.id),
        "archived board head must round-trip and remain archived"
    );
    assert_eq!(
        app2.model.archived_boards().len(),
        1,
        "exactly one archived_boards marker must survive"
    );

    // The archived board's SUBTREE must round-trip. Assert against the raw
    // backend snapshot (the model's live-scoped views exclude archived-board
    // descendants).
    let snap = app2.ctx.snapshot().unwrap();
    assert!(
        snap.boards.iter().any(|b| b.id == arch_board.id),
        "archived board head must be in the re-imported snapshot.boards"
    );
    assert!(
        snap.columns.iter().any(|c| c.id == arch_col.id),
        "archived board's column must round-trip"
    );
    assert!(
        snap.cards.iter().any(|c| c.id == arch_card.id),
        "archived board's card must round-trip"
    );
    assert!(
        snap.sprints.iter().any(|s| s.id == arch_sprint.id),
        "archived board's sprint must round-trip"
    );
    assert!(
        snap.archived_boards
            .iter()
            .any(|ab| ab.entity_id == arch_board.id),
        "archived_boards marker must reference the re-imported board head (no orphan)"
    );
}

#[test]
fn test_tui_auto_save_round_trips_archived_board_with_subtree() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("autosave_archived.json");

    let mut app = App::test_default();
    app.persistence.save_file = Some(file_path.to_str().unwrap().to_string());

    let arch_board = app
        .ctx
        .create_board("Archived Board".to_string(), None)
        .unwrap();
    let arch_col = app
        .ctx
        .create_column(arch_board.id, "Backlog".to_string(), None)
        .unwrap();
    let arch_card = app
        .ctx
        .create_card(
            arch_board.id,
            arch_col.id,
            "Card".to_string(),
            Default::default(),
        )
        .unwrap();
    app.ctx.archive_board(arch_board.id).unwrap();
    app.reload_model();
    app.prepare_frame();

    // Drive the ACTUAL auto-save entry point.
    app.auto_save().unwrap();

    let mut app2 = App::test_default();
    app2.import_board_from_file(file_path.to_str().unwrap())
        .unwrap();
    app2.reload_model();
    app2.prepare_frame();

    assert_eq!(
        app2.model.archived_boards().len(),
        1,
        "archived_boards marker must survive auto_save round-trip"
    );
    let snap = app2.ctx.snapshot().unwrap();
    assert!(
        snap.boards.iter().any(|b| b.id == arch_board.id),
        "archived board head must survive auto_save round-trip"
    );
    assert!(
        snap.columns.iter().any(|c| c.id == arch_col.id),
        "archived board column must survive auto_save round-trip"
    );
    assert!(
        snap.cards.iter().any(|c| c.id == arch_card.id),
        "archived board card must survive auto_save round-trip"
    );
    assert!(
        snap.archived_boards
            .iter()
            .any(|ab| ab.entity_id == arch_board.id),
        "archived_boards marker must not be orphaned after auto_save round-trip"
    );
}

#[test]
fn test_export_does_not_call_the_whole_store_trait_method() {
    let dir = tempdir().unwrap();

    let mut counted_app = App::test_default();
    counted_app
        .ctx
        .create_board("Board".to_string(), None)
        .unwrap();
    let (backend, snapshot_reads) = SnapshotCountingBackend::wrap(counted_app.ctx.backend());
    counted_app.ctx.replace_backend(backend);

    let counted_path = dir.path().join("counted.json");
    counted_app
        .input
        .set(counted_path.to_str().unwrap().to_string());
    counted_app.export_all_boards_with_filename().unwrap();
    assert_eq!(
        snapshot_reads.load(Ordering::SeqCst),
        0,
        "export must not read the whole-store snapshot"
    );

    let mut failing_app = App::test_default();
    failing_app
        .ctx
        .create_board("Board".to_string(), None)
        .unwrap();
    let failing_backend = FailingSnapshotBackend::wrap(failing_app.ctx.backend());
    failing_app.ctx.replace_backend(failing_backend);

    let failing_path = dir.path().join("failing.json");
    failing_app
        .input
        .set(failing_path.to_str().unwrap().to_string());
    assert!(
        failing_app.export_all_boards_with_filename().is_ok(),
        "export must succeed even when the whole-store snapshot fails"
    );
}

#[test]
fn test_export_all_boards_round_trips_an_archived_board_subtree() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("export_all_full_graph.json");

    let mut app = App::test_default();

    let live_board = app
        .ctx
        .create_board("Live Board".to_string(), None)
        .unwrap();
    let live_col = app
        .ctx
        .create_column(live_board.id, "Todo".to_string(), None)
        .unwrap();
    let live_card = app
        .ctx
        .create_card(
            live_board.id,
            live_col.id,
            "Live Card".to_string(),
            Default::default(),
        )
        .unwrap();
    let child_card = app
        .ctx
        .create_card(
            live_board.id,
            live_col.id,
            "Child Card".to_string(),
            Default::default(),
        )
        .unwrap();
    app.ctx.attach_child(live_card.id, child_card.id).unwrap();

    let archived_on_live_card = app
        .ctx
        .create_card(
            live_board.id,
            live_col.id,
            "Archived On Live".to_string(),
            Default::default(),
        )
        .unwrap();
    app.ctx.archive_card(archived_on_live_card.id).unwrap();

    let arch_board = app
        .ctx
        .create_board("Archived Board".to_string(), None)
        .unwrap();
    let arch_col = app
        .ctx
        .create_column(arch_board.id, "Backlog".to_string(), None)
        .unwrap();
    let arch_card = app
        .ctx
        .create_card(
            arch_board.id,
            arch_col.id,
            "Archived Board Card".to_string(),
            Default::default(),
        )
        .unwrap();
    let arch_sprint = app
        .ctx
        .create_sprint(arch_board.id, Some("S".to_string()), None)
        .unwrap();
    app.ctx.archive_board(arch_board.id).unwrap();
    app.reload_model();
    app.prepare_frame();

    app.input.set(file_path.to_str().unwrap().to_string());
    app.export_all_boards_with_filename().unwrap();

    let mut app2 = App::test_default();
    app2.import_board_from_file(file_path.to_str().unwrap())
        .unwrap();
    app2.reload_model();
    app2.prepare_frame();

    assert!(
        app2.model
            .boards_state()
            .loaded_or_empty()
            .iter()
            .any(|b| b.id == live_board.id),
        "live board head must round-trip"
    );
    assert!(
        app2.model
            .boards_state()
            .loaded_or_empty()
            .iter()
            .any(|b| b.id == arch_board.id),
        "archived board head must round-trip"
    );

    let snap = app2.ctx.snapshot().unwrap();

    let re_live_col = snap
        .columns
        .iter()
        .find(|c| c.id == live_col.id)
        .expect("live board column must round-trip");
    assert_eq!(re_live_col.position, live_col.position, "column position");
    assert_eq!(re_live_col.wip_limit, live_col.wip_limit, "column wip_limit");

    let re_arch_col = snap
        .columns
        .iter()
        .find(|c| c.id == arch_col.id)
        .expect("archived board column must round-trip");
    assert_eq!(re_arch_col.position, arch_col.position, "archived column position");
    assert_eq!(
        re_arch_col.wip_limit, arch_col.wip_limit,
        "archived column wip_limit"
    );

    let re_live_card = snap
        .cards
        .iter()
        .find(|c| c.id == live_card.id)
        .expect("live card must round-trip");
    assert_eq!(re_live_card.position, live_card.position, "card position");
    assert_eq!(re_live_card.prefix, live_card.prefix, "card prefix");
    assert_eq!(
        re_live_card.card_number, live_card.card_number,
        "card card_number"
    );

    let re_arch_card = snap
        .cards
        .iter()
        .find(|c| c.id == arch_card.id)
        .expect("archived board card must round-trip");
    assert_eq!(
        re_arch_card.position, arch_card.position,
        "archived board card position"
    );

    assert!(
        snap.cards.iter().any(|c| c.id == archived_on_live_card.id),
        "archived card's live row must round-trip"
    );
    assert!(
        snap.archived_cards
            .iter()
            .any(|ac| ac.entity_id == archived_on_live_card.id),
        "archived card's marker must round-trip"
    );

    let re_arch_sprint = snap
        .sprints
        .iter()
        .find(|s| s.id == arch_sprint.id)
        .expect("archived board sprint must round-trip");
    assert_eq!(
        re_arch_sprint.sprint_number, arch_sprint.sprint_number,
        "sprint sprint_number"
    );

    assert!(
        snap.archived_boards
            .iter()
            .any(|ab| ab.entity_id == arch_board.id),
        "archived_boards marker must round-trip"
    );

    assert!(
        app2.ctx.list_children_of(live_card.id).unwrap().is_empty(),
        "AllBoardsExport carries no graph field, so dependency edges do not round-trip"
    );
}
