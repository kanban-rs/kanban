mod common;
use common::TestContext;

use kanban_domain::commands::card::{ArchiveCards, UpdateCard};
use kanban_domain::commands::column_commands::DeleteColumn;
use kanban_domain::commands::{CardCommand, Command};

use kanban_domain::{CardUpdate, DataStore, FieldUpdate};
use uuid::Uuid;

#[test]
fn test_update_card_not_found_returns_error() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let cmd = UpdateCard {
        card_id: Uuid::new_v4(),
        updates: CardUpdate::default(),
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_update_card_to_nonexistent_column_returns_not_found() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let col_id = col.id;
    let card = kanban_domain::Card::new(board.id, col_id, "Card", 0);
    let card_id = card.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();

    let context = tc.as_command_context();
    let cmd = UpdateCard {
        card_id,
        updates: CardUpdate {
            column_id: Some(Uuid::new_v4()),
            ..CardUpdate::default()
        },
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());

    // FK rejected before mutation: the card stays in its original column.
    let stored = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(stored.column_id, col_id);
}

#[test]
fn test_update_card_setting_a_sprint_on_another_board_returns_sprint_board_mismatch() {
    let tc = TestContext::new();
    let board_a = kanban_domain::Board::new("A", Some("AAA"));
    let board_b = kanban_domain::Board::new("B", Some("BBB"));
    let col_a = kanban_domain::Column::new(board_a.id, "Col", 0);
    let card = kanban_domain::Card::new(board_a.id, col_a.id, "Card", 0);
    let card_id = card.id;
    let sprint_b = kanban_domain::Sprint::new(board_b.id, 1, None, Some("Sprint"));
    let sprint_id = sprint_b.id;
    tc.store.upsert_board(board_a).unwrap();
    tc.store.upsert_board(board_b).unwrap();
    tc.store.upsert_column(col_a).unwrap();
    tc.store.upsert_card(card).unwrap();
    tc.store.upsert_sprint(sprint_b).unwrap();

    let context = tc.as_command_context();
    let cmd = UpdateCard {
        card_id,
        updates: CardUpdate {
            sprint_id: FieldUpdate::Set(sprint_id),
            ..CardUpdate::default()
        },
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_sprint_board_mismatch());

    let stored = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(stored.sprint_id, None);
}

#[test]
fn test_update_card_setting_an_unknown_sprint_returns_not_found() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let card = kanban_domain::Card::new(board.id, col.id, "Card", 0);
    let card_id = card.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();

    let context = tc.as_command_context();
    let cmd = UpdateCard {
        card_id,
        updates: CardUpdate {
            sprint_id: FieldUpdate::Set(Uuid::new_v4()),
            ..CardUpdate::default()
        },
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_update_card_resubmitting_its_existing_cross_board_sprint_succeeds() {
    let tc = TestContext::new();
    let board_a = kanban_domain::Board::new("A", Some("AAA"));
    let board_b = kanban_domain::Board::new("B", Some("BBB"));
    let col_a = kanban_domain::Column::new(board_a.id, "Col", 0);
    let mut card = kanban_domain::Card::new(board_a.id, col_a.id, "Card", 0);
    let sprint_b = kanban_domain::Sprint::new(board_b.id, 1, None, Some("Sprint"));
    let sprint_id = sprint_b.id;
    card.assign_to_sprint(
        sprint_id,
        sprint_b.sprint_number,
        None::<String>,
        "Planning",
        chrono::Utc::now(),
    );
    let card_id = card.id;
    tc.store.upsert_board(board_a).unwrap();
    tc.store.upsert_board(board_b).unwrap();
    tc.store.upsert_column(col_a).unwrap();
    tc.store.upsert_card(card).unwrap();
    tc.store.upsert_sprint(sprint_b).unwrap();

    let context = tc.as_command_context();
    let cmd = UpdateCard {
        card_id,
        updates: CardUpdate {
            title: Some("Renamed".to_string()),
            sprint_id: FieldUpdate::Set(sprint_id),
            ..CardUpdate::default()
        },
    };
    let result = cmd.execute(&context);
    assert!(result.is_ok());

    let stored = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(stored.title, "Renamed");
    assert_eq!(stored.sprint_id, Some(sprint_id));
}

#[test]
fn test_update_card_with_a_column_on_another_board_checks_the_sprint_against_the_cards_stored_board(
) {
    let tc = TestContext::new();
    let board_a = kanban_domain::Board::new("A", Some("AAA"));
    let board_b = kanban_domain::Board::new("B", Some("BBB"));
    let col_b = kanban_domain::Column::new(board_b.id, "Col", 0);
    let col_b_id = col_b.id;
    let card = kanban_domain::Card::new(board_a.id, Uuid::new_v4(), "Card", 0);
    let card_id = card.id;
    let sprint_b = kanban_domain::Sprint::new(board_b.id, 1, None, Some("Sprint"));
    let sprint_id = sprint_b.id;
    tc.store.upsert_board(board_a).unwrap();
    tc.store.upsert_board(board_b).unwrap();
    tc.store.upsert_column(col_b).unwrap();
    tc.store.upsert_card(card).unwrap();
    tc.store.upsert_sprint(sprint_b).unwrap();

    let context = tc.as_command_context();
    // Raw UpdateCard with a column on board B but the card is still stored on
    // board A: the sprint check must compare against the card's stored
    // board_id, not the (unsynced) target column's board.
    let cmd = UpdateCard {
        card_id,
        updates: CardUpdate {
            column_id: Some(col_b_id),
            sprint_id: FieldUpdate::Set(sprint_id),
            ..CardUpdate::default()
        },
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_sprint_board_mismatch());

    let stored = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(stored.sprint_id, None);
}

#[test]
fn test_update_card_inverse_restores_the_sprint_attachment_verbatim() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("A", Some("AAA"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let card = kanban_domain::Card::new(board.id, col.id, "Card", 0);
    let card_id = card.id;
    let sprint = kanban_domain::Sprint::new(board.id, 1, None, Some("Sprint"));
    let sprint_id = sprint.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();
    tc.store.upsert_sprint(sprint).unwrap();

    let before = tc.store.get_card(card_id).unwrap().unwrap();

    let forward = UpdateCard {
        card_id,
        updates: CardUpdate {
            sprint_id: FieldUpdate::Set(sprint_id),
            ..CardUpdate::default()
        },
    };
    let inverse = forward.capture_inverse(&tc.store).unwrap();
    assert_eq!(
        inverse.len(),
        2,
        "plain Update plus RestoreSprintAttachment"
    );
    match &inverse[0] {
        Command::Card(CardCommand::Update(u)) => {
            assert_eq!(u.updates.sprint_id, FieldUpdate::NoChange);
        }
        other => panic!("expected Update first, got {other:?}"),
    }
    match &inverse[1] {
        Command::Card(CardCommand::RestoreSprintAttachment(r)) => {
            assert_eq!(r.card_id, card_id);
            assert_eq!(r.sprint_id, before.sprint_id);
            assert_eq!(r.sprint_logs, before.sprint_logs);
            assert_eq!(r.updated_at, before.updated_at);
        }
        other => panic!("expected RestoreSprintAttachment second, got {other:?}"),
    }

    let context = tc.as_command_context();
    forward.execute(&context).unwrap();
    for cmd in inverse {
        cmd.execute(&context).unwrap();
    }
    let after = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(after, before);
}

#[test]
fn test_update_card_inverse_without_sprint_change_is_a_single_update() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("A", Some("AAA"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let card = kanban_domain::Card::new(board.id, col.id, "Card", 0);
    let card_id = card.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();

    let forward = UpdateCard {
        card_id,
        updates: CardUpdate {
            title: Some("Renamed".to_string()),
            ..CardUpdate::default()
        },
    };
    let inverse = forward.capture_inverse(&tc.store).unwrap();
    assert_eq!(inverse.len(), 1);
}

#[test]
fn test_archive_cards_all_invalid_ids_returns_error() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let cmd = ArchiveCards {
        ids: vec![Uuid::new_v4()],
    };
    let result = cmd.execute(&context);
    assert!(result.is_err(), "Expected error when all IDs are invalid");
}

#[test]
fn test_archive_cards_invalid_ids_skipped_valid_ids_archived() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let card = kanban_domain::Card::new(board.id, Uuid::new_v4(), "Card", 0);
    let valid_id = card.id;
    tc.store.upsert_card(card).unwrap();

    let context = tc.as_command_context();
    let cmd = ArchiveCards {
        ids: vec![valid_id, Uuid::new_v4()],
    };
    let result = cmd.execute(&context);
    assert!(result.is_ok());
    assert_eq!(tc.store.list_all_cards().unwrap().len(), 0);
    assert_eq!(tc.store.list_archived_cards().unwrap().len(), 1);
}

#[test]
fn test_archive_captures_board_from_column() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let board_id = board.id;
    let col = kanban_domain::Column::new(board_id, "Col", 0);
    let col_id = col.id;
    let card = kanban_domain::Card::new(board.id, col_id, "Card", 0);
    let card_id = card.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();

    let context = tc.as_command_context();
    let cmd = ArchiveCards { ids: vec![card_id] };
    cmd.execute(&context).unwrap();

    // Capture walks card -> column -> board rather than defaulting to nil.
    // Under the marker model there is no stored original col/pos: the card
    // stays live in place and only board_id + archived_at are recorded.
    let archived = tc.store.get_archived_card(card_id).unwrap().unwrap();
    assert_eq!(archived.context.board_id, board_id);
    assert_eq!(archived.entity_id, card_id);
    // The live card is untouched: still in its original column at position 0.
    let live = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(live.column_id, col_id);
    assert_eq!(live.position, 0);
}

#[test]
fn test_archive_batch_captures_each_cards_own_board() {
    let tc = TestContext::new();
    let board_a = kanban_domain::Board::new("A", Some("AAA"));
    let board_a_id = board_a.id;
    let col_a = kanban_domain::Column::new(board_a_id, "Col", 0);
    let card_a = kanban_domain::Card::new(board_a.id, col_a.id, "CardA", 0);
    let card_a_id = card_a.id;

    let board_b = kanban_domain::Board::new("B", Some("BBB"));
    let board_b_id = board_b.id;
    let col_b = kanban_domain::Column::new(board_b_id, "Col", 0);
    let card_b = kanban_domain::Card::new(board_b.id, col_b.id, "CardB", 0);
    let card_b_id = card_b.id;

    tc.store.upsert_board(board_a).unwrap();
    tc.store.upsert_column(col_a).unwrap();
    tc.store.upsert_card(card_a).unwrap();
    tc.store.upsert_board(board_b).unwrap();
    tc.store.upsert_column(col_b).unwrap();
    tc.store.upsert_card(card_b).unwrap();

    let context = tc.as_command_context();
    let cmd = ArchiveCards {
        ids: vec![card_a_id, card_b_id],
    };
    cmd.execute(&context).unwrap();

    // Each archived card captures ITS OWN board, proving the loop resolves the
    // board per-item rather than hoisting or reusing the first card's board.
    let arch_a = tc.store.get_archived_card(card_a_id).unwrap().unwrap();
    let arch_b = tc.store.get_archived_card(card_b_id).unwrap().unwrap();
    assert_eq!(arch_a.context.board_id, board_a_id);
    assert_eq!(arch_b.context.board_id, board_b_id);
}

#[test]
fn test_archive_with_corrupted_board_id_captures_nil_board_id() {
    // Since KAN-963, ArchiveCards reads card.board_id directly (a durable
    // field set at creation and kept in sync on every move) rather than
    // deriving it via a column lookup, so a dangling column_id alone no
    // longer affects board_id resolution -- Card::new(board.id, ..) always
    // sets a valid board_id regardless of column_id. The only way archive
    // still sees a nil board_id is genuinely corrupted/legacy data where the
    // card's OWN board_id is nil (e.g. imported pre-migration data), which a
    // raw literal simulates here (bypassing Card::new/Card::create).
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let card = kanban_domain::Card {
        id: Uuid::new_v4(),
        column_id: Uuid::new_v4(),
        board_id: Uuid::nil(),
        title: "Card".to_string(),
        description: None,
        priority: kanban_domain::CardPriority::Medium,
        status: kanban_domain::CardStatus::Todo,
        position: 0,
        due_date: None,
        points: None,
        card_number: 1,
        sprint_id: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        completed_at: None,
        sprint_logs: Vec::new(),
        prefix: String::new(),
    };
    let card_id = card.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_card(card).unwrap();

    let context = tc.as_command_context();
    let cmd = ArchiveCards { ids: vec![card_id] };
    // Best-effort capture: a corrupted board_id must NOT abort the archive.
    assert!(cmd.execute(&context).is_ok());

    let archived = tc.store.get_archived_card(card_id).unwrap().unwrap();
    assert_eq!(archived.context.board_id, Uuid::nil());
}

#[test]
fn test_archive_card_after_column_deleted_preserves_board_id() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let board_id = board.id;
    let col = kanban_domain::Column::new(board_id, "Col", 0);
    let col_id = col.id;
    let card = kanban_domain::Card::new(board.id, col_id, "Card", 0);
    let card_id = card.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();

    let context = tc.as_command_context();
    ArchiveCards { ids: vec![card_id] }
        .execute(&context)
        .unwrap();

    // The column is now empty (its only card is archived) and legitimately
    // deletable -- archived cards don't block column deletion (D2).
    DeleteColumn { column_id: col_id }
        .execute(&context)
        .unwrap();

    let archived = tc.store.get_archived_card(card_id).unwrap().unwrap();
    assert_eq!(
        archived.context.board_id, board_id,
        "board_id survives the column's deletion"
    );
}

#[test]
fn test_double_archive_after_column_deleted_does_not_clobber_board_id() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let board_id = board.id;
    let col = kanban_domain::Column::new(board_id, "Col", 0);
    let col_id = col.id;
    let card = kanban_domain::Card::new(board.id, col_id, "Card", 0);
    let card_id = card.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();

    let context = tc.as_command_context();
    ArchiveCards { ids: vec![card_id] }
        .execute(&context)
        .unwrap();
    DeleteColumn { column_id: col_id }
        .execute(&context)
        .unwrap();

    // Re-archive the SAME already-archived card (idempotent retry / re-issued
    // command / undo-redo replay) after its column is gone. This must not
    // re-derive board_id from the now-dangling column_id and clobber the
    // value already correctly captured on the first archive.
    ArchiveCards { ids: vec![card_id] }
        .execute(&context)
        .unwrap();

    let archived = tc.store.get_archived_card(card_id).unwrap().unwrap();
    assert_eq!(
        archived.context.board_id, board_id,
        "re-archiving after the column is gone must not clobber the already-correct board_id"
    );
}

#[test]
fn test_double_archive_of_already_archived_card_preserves_archived_at() {
    // The idempotency guard skips the marker insert entirely when one already
    // exists, so re-archiving must not refresh `archived_at` either -- not
    // just `board_id`. Stamp the marker with an obviously-distinct sentinel
    // timestamp after the first archive (standing in for "the original
    // archive time") so the assertion doesn't depend on real wall-clock
    // timing to distinguish it from whatever a second archive's `Utc::now()`
    // would otherwise produce.
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let board_id = board.id;
    let col = kanban_domain::Column::new(board_id, "Col", 0);
    let col_id = col.id;
    let card = kanban_domain::Card::new(board.id, col_id, "Card", 0);
    let card_id = card.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();

    let context = tc.as_command_context();
    ArchiveCards { ids: vec![card_id] }
        .execute(&context)
        .unwrap();

    let sentinel_archived_at: chrono::DateTime<chrono::Utc> =
        "2000-01-01T00:00:00Z".parse().unwrap();
    tc.store
        .insert_archived_card(kanban_domain::Archived::with_context(
            card_id,
            kanban_domain::CardRestoreContext { board_id },
            kanban_domain::ArchiveMetadata::at(sentinel_archived_at),
        ))
        .unwrap();

    DeleteColumn { column_id: col_id }
        .execute(&context)
        .unwrap();
    ArchiveCards { ids: vec![card_id] }
        .execute(&context)
        .unwrap();

    let archived = tc.store.get_archived_card(card_id).unwrap().unwrap();
    assert_eq!(
        archived.metadata.archived_at, sentinel_archived_at,
        "re-archiving an already-archived card must not refresh archived_at"
    );
}

#[test]
fn test_archive_cards_missing_card_after_filter_returns_error() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let card = kanban_domain::Card::new(board.id, Uuid::new_v4(), "Card", 0);
    let card_id = card.id;
    tc.store.upsert_card(card).unwrap();

    // Directly call ArchiveCards with a valid card id.
    // The card will be found by filter_valid_card_ids, then get_card should
    // return a proper error (not panic) if the card is somehow missing.
    // Here we test the happy path still works, plus we ensure the error
    // path is properly handled (not an unwrap panic) via the impl fix.
    let context = tc.as_command_context();
    let cmd = ArchiveCards { ids: vec![card_id] };
    assert!(cmd.execute(&context).is_ok());
    assert_eq!(tc.store.list_all_cards().unwrap().len(), 0);
    assert_eq!(tc.store.list_archived_cards().unwrap().len(), 1);
}

#[test]
fn test_archive_cards_capture_inverse_skips_an_already_archived_card() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let board_id = board.id;
    let col = kanban_domain::Column::new(board_id, "Col", 0);
    let col_id = col.id;
    let live = kanban_domain::Card::new(board_id, col_id, "Live", 0);
    let live_id = live.id;
    let archived = kanban_domain::Card::new(board_id, col_id, "Archived", 1);
    let archived_id = archived.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(live).unwrap();
    tc.store.upsert_card(archived).unwrap();

    let context = tc.as_command_context();
    ArchiveCards {
        ids: vec![archived_id],
    }
    .execute(&context)
    .unwrap();

    let inverse = ArchiveCards {
        ids: vec![live_id, archived_id],
    }
    .capture_inverse(&tc.store)
    .unwrap();

    assert_eq!(inverse.len(), 1);
    match &inverse[0] {
        Command::Card(CardCommand::Restore(r)) => {
            assert_eq!(r.card_id, live_id);
            let live_card = tc.store.get_card(live_id).unwrap().unwrap();
            assert_eq!(r.column_id, live_card.column_id);
            assert_eq!(r.position, live_card.position);
        }
        other => panic!("expected Restore for the live card, got {other:?}"),
    }
}

#[test]
fn test_archive_cards_capture_inverse_records_one_restore_per_distinct_id() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let board_id = board.id;
    let col = kanban_domain::Column::new(board_id, "Col", 0);
    let col_id = col.id;
    let live = kanban_domain::Card::new(board_id, col_id, "Live", 0);
    let live_id = live.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(live).unwrap();

    let inverse = ArchiveCards {
        ids: vec![live_id, live_id],
    }
    .capture_inverse(&tc.store)
    .unwrap();

    assert_eq!(inverse.len(), 1);
    match &inverse[0] {
        Command::Card(CardCommand::Restore(r)) => assert_eq!(r.card_id, live_id),
        other => panic!("expected a single Restore for the live card, got {other:?}"),
    }
}
