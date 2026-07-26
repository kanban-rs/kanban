//! KAN-895 regression: the TUI File→Export-All path must fully round-trip an
//! archived board — its HEAD, its entire subtree (columns / cards / sprints,
//! which live in the flat collections under the archived board_id), AND its
//! `archived_boards` marker (so it re-imports still archived, hidden from the
//! live list).
//!
//! Before the fix, `export_all_boards_with_filename` / `auto_save` read the
//! live-scoped `model.boards()` / `model.all_cards()`, so an archived board's head
//! and subtree were omitted and the exported `archived_boards` marker referenced
//! a board absent from the file → orphan / silent data loss on re-import. These
//! tests drive the ACTUAL TUI export entry point (not the snapshot path
//! directly).

use kanban_domain::KanbanOperations;
use kanban_tui::App;
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
    app.prepare_frame();

    // Sanity: the archived board head is hidden from the live list, present in
    // the archived-boards view.
    assert!(
        app.model.live_boards().all(|b| b.id != arch_board.id),
        "archived board must be hidden from live list before export"
    );
    assert!(
        app.model.archived_board_ids().contains(&arch_board.id),
        "archived board head must be in the archived set before export"
    );

    // Drive the ACTUAL TUI File→Export-All entry point.
    app.input.set(file_path.to_str().unwrap().to_string());
    app.prepare_frame();
    app.export_all_boards_with_filename().unwrap();

    // Re-import into a FRESH app and assert the whole archived-board graph is back.
    let mut app2 = App::test_default();
    app2.import_board_from_file(file_path.to_str().unwrap())
        .unwrap();
    app2.prepare_frame();

    // Live board still live.
    assert!(
        app2.model.boards().iter().any(|b| b.id == live_board.id),
        "live board must be present after re-import"
    );

    // Archived board head is back AND still archived (hidden from live list,
    // present in the archived view with its marker).
    assert!(
        app2.model.live_boards().all(|b| b.id != arch_board.id),
        "archived board must remain hidden from live list after re-import"
    );
    assert!(
        app2.model.boards().iter().any(|b| b.id == arch_board.id)
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
    app.prepare_frame();

    // Drive the ACTUAL auto-save entry point.
    app.auto_save().unwrap();

    let mut app2 = App::test_default();
    app2.import_board_from_file(file_path.to_str().unwrap())
        .unwrap();
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
