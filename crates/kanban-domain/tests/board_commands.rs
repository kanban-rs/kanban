mod common;
use common::TestContext;
use uuid::Uuid;

use kanban_domain::commands::board_commands::*;
use kanban_domain::DataStore;
use kanban_domain::*;

#[test]
fn test_create_board_command_funnels_through_factory_with_injected_id() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let cmd = CreateBoard {
        id,
        name: "Factory Funnel".to_string(),
        card_prefix: Some("KAN".to_string()),
        position: 3,
    };
    cmd.execute(&context).unwrap();

    let board = tc.store.get_board(id).unwrap().unwrap();
    assert_eq!(board.id, id);
    assert_eq!(board.name, "Factory Funnel");
    assert_eq!(board.card_prefix, Some("KAN".to_string()));
    // Server-managed position applied verbatim, counters seeded by the factory:
    assert_eq!(board.position, 3);
    assert_eq!(board.next_sprint_number, 1);
    // Factory uses a single clock for both timestamps:
    assert_eq!(board.created_at, board.updated_at);
}

#[test]
fn test_create_board_command_rejects_blank_name_via_factory_validation() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let cmd = CreateBoard {
        id: Uuid::new_v4(),
        name: "   ".to_string(),
        card_prefix: None,
        position: 0,
    };
    let err = cmd.execute(&context).unwrap_err();
    assert!(err.is_validation());
}

#[test]
fn test_update_board_not_found_returns_error() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let cmd = UpdateBoard {
        board_id: Uuid::new_v4(),
        updates: kanban_domain::BoardUpdate::default(),
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_set_board_task_sort_not_found_returns_error() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let cmd = SetBoardTaskSort {
        board_id: Uuid::new_v4(),
        field: kanban_domain::SortField::Priority,
        order: kanban_domain::SortOrder::Ascending,
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_set_board_task_list_view_not_found_returns_error() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let cmd = SetBoardTaskListView {
        board_id: Uuid::new_v4(),
        view: kanban_domain::TaskListView::default(),
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_import_entities_with_duplicate_board_id_returns_error() {
    let tc = TestContext::new();
    let b1 = Board::new("B1", None::<String>);
    let dup_id = b1.id;
    tc.store.upsert_board(b1).unwrap();

    let mut dup = Board::new("Dup", None::<String>);
    dup.id = dup_id;

    let cmd = ImportEntities {
        boards: vec![dup],
        columns: vec![],
        cards: vec![],
        archived_cards: vec![],
        archived_boards: vec![],
        sprints: vec![],
        graph: None,
    };
    let context = tc.as_command_context();
    let result = cmd.execute(&context);
    assert!(result.is_err());
    assert!(result.unwrap_err().is_validation());
}

#[test]
fn test_import_entities_with_duplicate_card_id_returns_error() {
    let tc = TestContext::new();
    let board = Board::new("B", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let card = kanban_domain::Card::new(board.id, col.id, "Card", 0);
    let dup_card_id = card.id;
    tc.store.upsert_board(board.clone()).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();

    let mut dup_card = kanban_domain::Card::new(board.id, Uuid::new_v4(), "Dup", 0);
    dup_card.id = dup_card_id;

    let cmd = ImportEntities {
        boards: vec![],
        columns: vec![],
        cards: vec![dup_card],
        archived_cards: vec![],
        archived_boards: vec![],
        sprints: vec![],
        graph: None,
    };
    let context = tc.as_command_context();
    let result = cmd.execute(&context);
    assert!(result.is_err());
    assert!(result.unwrap_err().is_validation());
}

#[test]
fn test_import_entities_live_card_colliding_with_existing_archived_returns_error() {
    let tc = TestContext::new();
    let board = Board::new("B", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let archived = kanban_domain::Card::new(board.id, col.id, "Archived", 0);
    let collision_id = archived.id;
    tc.store.upsert_board(board.clone()).unwrap();
    tc.store.upsert_column(col.clone()).unwrap();
    tc.store
        .insert_archived_card(kanban_domain::ArchivedCard::new(
            archived.id,
            uuid::Uuid::nil(),
        ))
        .unwrap();

    let mut imported_live = kanban_domain::Card::new(board.id, col.id, "ImportedLive", 0);
    imported_live.id = collision_id;

    let cmd = ImportEntities {
        boards: vec![],
        columns: vec![],
        cards: vec![imported_live],
        archived_cards: vec![],
        archived_boards: vec![],
        sprints: vec![],
        graph: None,
    };
    let context = tc.as_command_context();
    let result = cmd.execute(&context);
    assert!(result.is_err());
    assert!(result.unwrap_err().is_validation());
}

#[test]
fn test_import_entities_archived_card_colliding_with_existing_live_returns_error() {
    let tc = TestContext::new();
    let board = Board::new("B", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let live = kanban_domain::Card::new(board.id, col.id, "Live", 0);
    let collision_id = live.id;
    tc.store.upsert_board(board.clone()).unwrap();
    tc.store.upsert_column(col.clone()).unwrap();
    tc.store.upsert_card(live).unwrap();

    let mut imported_archived = kanban_domain::Card::new(board.id, col.id, "ImportedArchived", 0);
    imported_archived.id = collision_id;

    let cmd = ImportEntities {
        boards: vec![],
        columns: vec![],
        cards: vec![],
        archived_cards: vec![kanban_domain::ArchivedCard::new(
            imported_archived.id,
            uuid::Uuid::nil(),
        )],
        archived_boards: vec![],
        sprints: vec![],
        graph: None,
    };
    let context = tc.as_command_context();
    let result = cmd.execute(&context);
    assert!(result.is_err());
    assert!(result.unwrap_err().is_validation());
}

#[test]
fn test_import_entities_with_duplicate_archived_card_id_returns_error() {
    let tc = TestContext::new();
    let board = Board::new("B", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let archived = kanban_domain::Card::new(board.id, col.id, "Archived", 0);
    let dup_id = archived.id;
    tc.store.upsert_board(board.clone()).unwrap();
    tc.store.upsert_column(col.clone()).unwrap();
    tc.store
        .insert_archived_card(kanban_domain::ArchivedCard::new(
            archived.id,
            uuid::Uuid::nil(),
        ))
        .unwrap();

    let mut dup = kanban_domain::Card::new(board.id, col.id, "Dup", 0);
    dup.id = dup_id;

    let cmd = ImportEntities {
        boards: vec![],
        columns: vec![],
        cards: vec![],
        archived_cards: vec![kanban_domain::ArchivedCard::new(dup.id, uuid::Uuid::nil())],
        archived_boards: vec![],
        sprints: vec![],
        graph: None,
    };
    let context = tc.as_command_context();
    let result = cmd.execute(&context);
    assert!(result.is_err());
    assert!(result.unwrap_err().is_validation());
}

#[test]
fn test_import_entities_appends_without_replacing() {
    let tc = TestContext::new();
    let b1 = Board::new("B1", None::<String>);
    tc.store.upsert_board(b1).unwrap();

    let b2 = Board::new("B2", None::<String>);
    let col = kanban_domain::Column::new(b2.id, "Todo", 0);
    let b2_clone = b2.clone();
    let card = kanban_domain::Card::new(b2_clone.id, col.id, "Card", 0);

    let cmd = ImportEntities {
        boards: vec![b2],
        columns: vec![col],
        cards: vec![card],
        archived_cards: vec![],
        archived_boards: vec![],
        sprints: vec![],
        graph: None,
    };

    let context = tc.as_command_context();
    cmd.execute(&context).unwrap();

    let boards = tc.store.list_boards().unwrap();
    assert_eq!(boards.len(), 2);
    assert!(boards.iter().any(|b| b.name == "B1"));
    assert!(boards.iter().any(|b| b.name == "B2"));
    assert_eq!(tc.store.list_all_columns().unwrap().len(), 1);
    assert_eq!(tc.store.list_all_cards().unwrap().len(), 1);
}

#[test]
fn test_update_board_card_prefix_allowed_before_first_card_succeeds() {
    let tc = TestContext::new();
    let board = Board::new("B", Some("OLD"));
    let board_id = board.id;
    tc.store.upsert_board(board).unwrap();
    let context = tc.as_command_context();

    let cmd = UpdateBoard {
        board_id,
        updates: kanban_domain::BoardUpdate {
            card_prefix: FieldUpdate::Set("NEW".to_string()),
            ..Default::default()
        },
    };
    assert!(cmd.execute(&context).is_ok());
    let board = tc.store.get_board(board_id).unwrap().unwrap();
    assert_eq!(board.card_prefix, Some("NEW".to_string()));
}

#[test]
fn test_update_board_card_prefix_after_cards_exist_is_allowed_and_leaves_them_alone() {
    let tc = TestContext::new();
    let board = Board::new("B", Some("OLD"));
    let board_id = board.id;
    let col = Column::new(board_id, "Col", 0);
    let mut card = Card::new(board.id, col.id, "C", 0);
    card.prefix = "OLD".to_string();
    let card_id = card.id;
    assert_eq!(
        card.prefix, "OLD",
        "the card is minted under the board's prefix, casing included"
    );
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();
    let context = tc.as_command_context();

    let cmd = UpdateBoard {
        board_id,
        updates: kanban_domain::BoardUpdate {
            card_prefix: FieldUpdate::Set("NEW".to_string()),
            ..Default::default()
        },
    };

    // Previously rejected, because a rename retroactively renamed every
    // existing card. A card now stores the prefix it was minted under, so the
    // rename is safe and affects only cards created afterwards.
    cmd.execute(&context).unwrap();

    let board = tc.store.get_board(board_id).unwrap().unwrap();
    assert_eq!(board.card_prefix, Some("NEW".to_string()));
    let card = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(
        card.prefix, "OLD",
        "the existing card keeps its identifier; the rename is not retroactive"
    );
}

#[test]
fn test_clear_board_card_prefix_after_cards_exist_is_allowed_and_leaves_them_alone() {
    let tc = TestContext::new();
    let board = Board::new("B", Some("OLD"));
    let board_id = board.id;
    let col = Column::new(board_id, "Col", 0);
    let mut card = Card::new(board.id, col.id, "C", 0);
    card.prefix = "OLD".to_string();
    let card_id = card.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();
    let context = tc.as_command_context();

    let cmd = UpdateBoard {
        board_id,
        updates: kanban_domain::BoardUpdate {
            card_prefix: FieldUpdate::Clear,
            ..Default::default()
        },
    };
    cmd.execute(&context).unwrap();

    let card = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(
        card.prefix, "OLD",
        "clearing the board's prefix does not strip identifiers off existing cards"
    );
}

#[test]
fn test_delete_board_atomic_removes_only_board_record() {
    let tc = TestContext::new();
    let board = Board::new("B", Some("TST"));
    let board_id = board.id;
    let col = Column::new(board_id, "Col", 0);
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col.clone()).unwrap();

    let context = tc.as_command_context();
    let cmd = DeleteBoard { board_id };
    cmd.execute(&context).unwrap();

    assert!(tc.store.list_boards().unwrap().is_empty());
    assert_eq!(
        tc.store.list_all_columns().unwrap().len(),
        1,
        "atomic DeleteBoard must not cascade to columns; cascade is the service's responsibility"
    );
}

// ===== C2: board archive / restore (collection move) =====

/// Seed a board with one column and one card; return (board_id, column_id,
/// card_id).
fn seed_board_with_subtree(tc: &TestContext) -> (Uuid, Uuid, Uuid) {
    let board = Board::new("B", Some("TST"));
    let board_id = board.id;
    let col = kanban_domain::Column::new(board_id, "Col", 0);
    let col_id = col.id;
    let card = kanban_domain::Card::new(board.id, col_id, "Task", 0);
    let card_id = card.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();
    (board_id, col_id, card_id)
}

#[test]
fn test_archive_boards_moves_board_from_live_to_archived_set() {
    let tc = TestContext::new();
    let (board_id, _, _) = seed_board_with_subtree(&tc);
    let ctx = tc.as_command_context();

    ArchiveBoards {
        ids: vec![board_id],
    }
    .execute(&ctx)
    .unwrap();

    assert!(
        tc.store.list_boards().unwrap().is_empty(),
        "archived board leaves the live set"
    );
    // Reference-marker model: the board head STAYS in `boards`; `get_board` is
    // unfiltered, so it still resolves (only the LIVE list hides it).
    assert!(tc.store.get_board(board_id).unwrap().is_some());
    let archived = tc.store.list_archived_boards().unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].entity_id, board_id);
}

#[test]
fn test_archive_board_leaves_subtree_columns_and_cards_in_place() {
    let tc = TestContext::new();
    let (board_id, col_id, card_id) = seed_board_with_subtree(&tc);
    let ctx = tc.as_command_context();

    ArchiveBoards {
        ids: vec![board_id],
    }
    .execute(&ctx)
    .unwrap();

    assert!(
        tc.store.get_column(col_id).unwrap().is_some(),
        "column stays in the flat collection"
    );
    assert!(
        tc.store.get_card(card_id).unwrap().is_some(),
        "card stays in the flat collection"
    );
}

#[test]
fn test_restore_board_moves_it_back_losslessly() {
    let tc = TestContext::new();
    let (board_id, _, _) = seed_board_with_subtree(&tc);
    let original = tc.store.get_board(board_id).unwrap().unwrap();
    let ctx = tc.as_command_context();

    ArchiveBoards {
        ids: vec![board_id],
    }
    .execute(&ctx)
    .unwrap();
    RestoreBoard { board_id }.execute(&ctx).unwrap();

    let back = tc.store.get_board(board_id).unwrap().unwrap();
    assert_eq!(back, original, "restore returns the board verbatim");
    assert!(tc.store.list_archived_boards().unwrap().is_empty());
}

#[test]
fn test_archive_then_undo_restores_board_identity() {
    let tc = TestContext::new();
    let (board_id, _, _) = seed_board_with_subtree(&tc);
    let original = tc.store.get_board(board_id).unwrap().unwrap();

    let forward = ArchiveBoards {
        ids: vec![board_id],
    };
    // Undo captures the inverse BEFORE the forward runs.
    let inverse = forward.capture_inverse(&tc.store).unwrap();
    let ctx = tc.as_command_context();
    forward.execute(&ctx).unwrap();
    assert!(
        tc.store.list_boards().unwrap().is_empty(),
        "archived: hidden from the live list"
    );

    for cmd in inverse {
        cmd.execute(&ctx).unwrap();
    }
    let back = tc.store.get_board(board_id).unwrap().unwrap();
    assert_eq!(back, original);
    assert!(tc.store.list_archived_boards().unwrap().is_empty());
}

#[test]
fn test_restore_then_undo_re_archives_board() {
    let tc = TestContext::new();
    let (board_id, _, _) = seed_board_with_subtree(&tc);
    let ctx = tc.as_command_context();
    ArchiveBoards {
        ids: vec![board_id],
    }
    .execute(&ctx)
    .unwrap();

    let forward = RestoreBoard { board_id };
    let inverse = forward.capture_inverse(&tc.store).unwrap();
    forward.execute(&ctx).unwrap();
    assert!(tc.store.get_board(board_id).unwrap().is_some());

    for cmd in inverse {
        cmd.execute(&ctx).unwrap();
    }
    assert!(
        tc.store.list_boards().unwrap().is_empty(),
        "re-archived by the inverse: hidden from the live list"
    );
    assert_eq!(tc.store.list_archived_boards().unwrap().len(), 1);
}

#[test]
fn test_archive_missing_board_returns_not_found() {
    let tc = TestContext::new();
    let ctx = tc.as_command_context();
    let result = ArchiveBoards {
        ids: vec![Uuid::new_v4()],
    }
    .execute(&ctx);
    assert!(result.is_err());
}

#[test]
fn test_import_board_colliding_with_archived_is_rejected() {
    let tc = TestContext::new();
    let (board_id, _, _) = seed_board_with_subtree(&tc);
    let ctx = tc.as_command_context();
    ArchiveBoards {
        ids: vec![board_id],
    }
    .execute(&ctx)
    .unwrap();

    // Import a fresh board whose id collides with the archived one.
    let mut colliding = Board::new("Colliding", Some("COL"));
    colliding.id = board_id;
    let cmd = ImportEntities {
        boards: vec![colliding],
        columns: vec![],
        cards: vec![],
        archived_cards: vec![],
        archived_boards: vec![],
        sprints: vec![],
        graph: None,
    };
    let result = cmd.execute(&ctx);
    assert!(
        result.is_err(),
        "must reject collision with an archived board"
    );
    assert!(result.unwrap_err().is_validation());
}

// ===== C3a: ImportEntities.archived_boards + collection-agnostic DeleteBoard =====

#[test]
fn test_import_entities_round_trips_archived_boards() {
    let tc = TestContext::new();
    let board = Board::new("B", Some("TST"));
    let id = board.id;
    // Reference-marker model: importing an archived board carries the board
    // head (into `.boards`) AND the marker (into `.archived_boards`).
    let cmd = ImportEntities {
        boards: vec![board.clone()],
        archived_boards: vec![kanban_domain::Archived::now(id)],
        ..Default::default()
    };
    let ctx = tc.as_command_context();
    cmd.execute(&ctx).unwrap();

    let archived = tc.store.list_archived_boards().unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].entity_id, id);
    assert!(
        tc.store.list_boards().unwrap().is_empty(),
        "archived board hidden from the live list"
    );
}

#[test]
fn test_import_archived_board_colliding_with_live_is_rejected() {
    let tc = TestContext::new();
    let (board_id, _, _) = seed_board_with_subtree(&tc);
    let mut colliding = Board::new("Dup", Some("DUP"));
    colliding.id = board_id;
    let cmd = ImportEntities {
        archived_boards: vec![kanban_domain::Archived::now(colliding.id)],
        ..Default::default()
    };
    let ctx = tc.as_command_context();
    let result = cmd.execute(&ctx);
    assert!(result.is_err());
    assert!(result.unwrap_err().is_validation());
}

#[test]
fn test_delete_board_removes_archived_board_from_collection() {
    let tc = TestContext::new();
    let (board_id, _, _) = seed_board_with_subtree(&tc);
    let ctx = tc.as_command_context();
    ArchiveBoards {
        ids: vec![board_id],
    }
    .execute(&ctx)
    .unwrap();
    assert_eq!(tc.store.list_archived_boards().unwrap().len(), 1);

    // Collection-agnostic DeleteBoard removes the record from the archived set.
    DeleteBoard { board_id }.execute(&ctx).unwrap();
    assert!(tc.store.list_archived_boards().unwrap().is_empty());
    assert!(tc.store.get_board(board_id).unwrap().is_none());
}

#[test]
fn test_delete_live_board_still_removes_from_live_set() {
    let tc = TestContext::new();
    let (board_id, _, _) = seed_board_with_subtree(&tc);
    let ctx = tc.as_command_context();
    DeleteBoard { board_id }.execute(&ctx).unwrap();
    assert!(tc.store.get_board(board_id).unwrap().is_none());
}

#[test]
fn test_delete_archived_board_inverse_reimports_as_archived() {
    let tc = TestContext::new();
    let (board_id, _, _) = seed_board_with_subtree(&tc);
    let ctx = tc.as_command_context();
    ArchiveBoards {
        ids: vec![board_id],
    }
    .execute(&ctx)
    .unwrap();

    // Undo of a permanent-delete of an archived board restores it AS archived.
    let del = DeleteBoard { board_id };
    let inverse = del.capture_inverse(&tc.store).unwrap();
    del.execute(&ctx).unwrap();
    assert!(tc.store.list_archived_boards().unwrap().is_empty());

    for cmd in inverse {
        cmd.execute(&ctx).unwrap();
    }
    assert!(
        tc.store.list_boards().unwrap().is_empty(),
        "restored as archived: hidden from the live set"
    );
    assert!(
        tc.store.get_board(board_id).unwrap().is_some(),
        "board head is present (unfiltered) but marked archived"
    );
    assert_eq!(tc.store.list_archived_boards().unwrap().len(), 1);
}

#[test]
fn test_delete_live_board_inverse_reimports_as_live() {
    let tc = TestContext::new();
    let (board_id, _, _) = seed_board_with_subtree(&tc);
    let ctx = tc.as_command_context();
    let del = DeleteBoard { board_id };
    let inverse = del.capture_inverse(&tc.store).unwrap();
    del.execute(&ctx).unwrap();

    for cmd in inverse {
        cmd.execute(&ctx).unwrap();
    }
    assert!(tc.store.get_board(board_id).unwrap().is_some());
    assert!(tc.store.list_archived_boards().unwrap().is_empty());
}

#[test]
fn test_delete_missing_board_inverse_errors() {
    let tc = TestContext::new();
    let result = DeleteBoard {
        board_id: Uuid::new_v4(),
    }
    .capture_inverse(&tc.store);
    assert!(result.is_err());
}
