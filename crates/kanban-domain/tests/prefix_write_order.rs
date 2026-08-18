mod common;

use common::prefix_write_order::PrefixWriteOrderStore;
use kanban_domain::commands::card::{
    ApplyCardMetadata, ArchiveCards, AssignCardsToSprint, CardCommand, CompactColumnPositions,
    CreateCard, MoveCard, RestoreCard, UnassignCardFromSprint, UpdateCard,
};
use kanban_domain::commands::dependency_commands::CreateSubcardCommand;
use kanban_domain::commands::{cascade_commands::SetArchivedCardsSprint, Command, CommandContext};
use kanban_domain::editable::CardMetadataDto;
use kanban_domain::{
    Board, Card, CardUpdate, Column, CreateCardOptions, DataStore, Prefix, Sprint,
};
use uuid::Uuid;

fn seed_board_column_card(store: &PrefixWriteOrderStore, prefix: &str) -> (Board, Column, Card) {
    let board = Board::new("B", Some(prefix));
    let column = Column::new(board.id, "Todo", 0);
    let (display, card_number) = kanban_domain::prefix::allocate_card_number(
        store,
        board.card_prefix.as_deref(),
        None,
        None,
    )
    .unwrap();
    let mut card = Card::new(board.id, column.id, "C", 0);
    card.card_number = card_number;
    card.prefix = display;

    store.upsert_board(board.clone()).unwrap();
    store.upsert_column(column.clone()).unwrap();
    store.upsert_card(card.clone()).unwrap();
    (board, column, card)
}

fn assert_all_backed(store: &PrefixWriteOrderStore) {
    let violations = store.unbacked_at_write();
    assert!(
        violations.is_empty(),
        "cards written while their namespace had no prefix row: {violations:?}"
    );
}

#[test]
fn test_creating_a_card_leaves_its_namespace_backed() {
    let store = PrefixWriteOrderStore::new();
    let board = Board::new("B", Some("KAN"));
    let column = Column::new(board.id, "Todo", 0);
    store.upsert_board(board.clone()).unwrap();
    store.upsert_column(column.clone()).unwrap();

    let default_card_prefix = "task".to_string();
    let (_display, card_number) = kanban_domain::prefix::allocate_card_number(
        &store,
        board.card_prefix.as_deref(),
        None,
        Some(&default_card_prefix),
    )
    .unwrap();

    let cmd = Command::Card(CardCommand::Create(CreateCard {
        id: Uuid::new_v4(),
        card_number,
        board_id: board.id,
        column_id: column.id,
        title: "New card".to_string(),
        position: 0,
        options: CreateCardOptions::default(),
        timestamp: chrono::Utc::now(),
        default_card_prefix,
    }));
    let context = CommandContext { store: &store };
    cmd.execute(&context).unwrap();

    assert_all_backed(&store);
}

#[test]
fn test_creating_a_subcard_leaves_its_namespace_backed() {
    let store = PrefixWriteOrderStore::new();
    let board = Board::new("B", Some("KAN"));
    let column = Column::new(board.id, "Todo", 0);
    let parent = Card::new(board.id, column.id, "Parent", 0);
    store.upsert_board(board.clone()).unwrap();
    store.upsert_column(column.clone()).unwrap();
    store.upsert_card(parent.clone()).unwrap();

    let cmd = Command::Dependency(kanban_domain::commands::DependencyCommand::CreateSubcard(
        CreateSubcardCommand {
            id: Uuid::new_v4(),
            parent_id: parent.id,
            board_id: board.id,
            column_id: column.id,
            title: "Subcard".to_string(),
            description: None,
            position: 0,
            default_card_prefix: "task".to_string(),
        },
    ));
    let context = CommandContext { store: &store };
    cmd.execute(&context).unwrap();

    assert_all_backed(&store);
}

#[test]
fn test_restoring_an_archived_card_leaves_its_namespace_backed() {
    let store = PrefixWriteOrderStore::new();
    let (_board, column, card) = seed_board_column_card(&store, "KAN");

    let context = CommandContext { store: &store };
    Command::Card(CardCommand::Archive(ArchiveCards { ids: vec![card.id] }))
        .execute(&context)
        .unwrap();

    let prefix_before = store.get_prefix("kan").unwrap().unwrap();

    Command::Card(CardCommand::Restore(RestoreCard {
        card_id: card.id,
        column_id: column.id,
        position: 0,
        timestamp: chrono::Utc::now(),
    }))
    .execute(&context)
    .unwrap();

    assert_all_backed(&store);
    let prefix_after = store.get_prefix("kan").unwrap().unwrap();
    assert_eq!(prefix_before.card_counter, prefix_after.card_counter);
}

#[test]
fn test_updating_a_card_leaves_its_namespace_backed() {
    let store = PrefixWriteOrderStore::new();
    let (_board, _column, card) = seed_board_column_card(&store, "KAN");
    let context = CommandContext { store: &store };

    Command::Card(CardCommand::Update(UpdateCard {
        card_id: card.id,
        updates: CardUpdate {
            title: Some("Renamed".to_string()),
            ..Default::default()
        },
    }))
    .execute(&context)
    .unwrap();

    assert_all_backed(&store);
}

#[test]
fn test_applying_card_metadata_leaves_its_namespace_backed() {
    let store = PrefixWriteOrderStore::new();
    let (_board, _column, card) = seed_board_column_card(&store, "KAN");
    let context = CommandContext { store: &store };

    Command::Card(CardCommand::ApplyMetadata(ApplyCardMetadata {
        card_id: card.id,
        dto: CardMetadataDto {
            priority: "High".to_string(),
            status: "InProgress".to_string(),
            points: Some(3),
            due_date: None,
        },
    }))
    .execute(&context)
    .unwrap();

    assert_all_backed(&store);
}

#[test]
fn test_moving_a_card_cross_board_leaves_its_namespace_backed() {
    let store = PrefixWriteOrderStore::new();
    let (_board, _column, card) = seed_board_column_card(&store, "KAN");

    let other_board = Board::new("Other", Some("OTH"));
    let other_column = Column::new(other_board.id, "Todo", 0);
    store.upsert_board(other_board.clone()).unwrap();
    store.upsert_column(other_column.clone()).unwrap();

    let context = CommandContext { store: &store };
    Command::Card(CardCommand::Move(MoveCard {
        card_id: card.id,
        new_column_id: other_column.id,
        new_position: 0,
    }))
    .execute(&context)
    .unwrap();

    assert_all_backed(&store);
    let moved = store.get_card(card.id).unwrap().unwrap();
    assert_eq!(moved.prefix, card.prefix);
}

#[test]
fn test_compacting_column_positions_leaves_its_namespace_backed() {
    let store = PrefixWriteOrderStore::new();
    let (board, column, _card) = seed_board_column_card(&store, "KAN");

    let (display, card_number) = kanban_domain::prefix::allocate_card_number(
        &store,
        board.card_prefix.as_deref(),
        None,
        None,
    )
    .unwrap();
    let mut second = Card::new(board.id, column.id, "C2", 5);
    second.card_number = card_number;
    second.prefix = display;
    store.upsert_card(second.clone()).unwrap();

    let context = CommandContext { store: &store };
    Command::Card(CardCommand::CompactPositions(CompactColumnPositions {
        column_id: column.id,
    }))
    .execute(&context)
    .unwrap();

    assert_all_backed(&store);
    let mut positions: Vec<i32> = store
        .list_cards_by_column(column.id)
        .unwrap()
        .into_iter()
        .map(|c| c.position)
        .collect();
    positions.sort();
    assert_eq!(positions, vec![0, 1]);
}

#[test]
fn test_assigning_cards_to_a_sprint_leaves_its_namespace_backed() {
    let store = PrefixWriteOrderStore::new();
    let (board, _column, card) = seed_board_column_card(&store, "KAN");
    let mut sprint = Sprint::new(board.id, 1, None, Some("SPR"));
    sprint.card_prefix = Some("SPR".to_string());
    store.upsert_sprint(sprint.clone()).unwrap();

    let context = CommandContext { store: &store };
    Command::Card(CardCommand::AssignToSprint(AssignCardsToSprint {
        ids: vec![card.id],
        sprint_id: sprint.id,
    }))
    .execute(&context)
    .unwrap();

    assert_all_backed(&store);
    let updated = store.get_card(card.id).unwrap().unwrap();
    assert_eq!(updated.prefix, card.prefix);
}

#[test]
fn test_unassigning_a_card_from_a_sprint_leaves_its_namespace_backed() {
    let store = PrefixWriteOrderStore::new();
    let (board, _column, card) = seed_board_column_card(&store, "KAN");
    let sprint = Sprint::new(board.id, 1, None, Some("SPR"));
    store.upsert_sprint(sprint.clone()).unwrap();
    let context = CommandContext { store: &store };
    Command::Card(CardCommand::AssignToSprint(AssignCardsToSprint {
        ids: vec![card.id],
        sprint_id: sprint.id,
    }))
    .execute(&context)
    .unwrap();

    Command::Card(CardCommand::UnassignFromSprint(UnassignCardFromSprint {
        card_id: card.id,
        timestamp: chrono::Utc::now(),
    }))
    .execute(&context)
    .unwrap();

    assert_all_backed(&store);
}

#[test]
fn test_restoring_a_card_sprint_attachment_leaves_its_namespace_backed() {
    let store = PrefixWriteOrderStore::new();
    let (_board, _column, card) = seed_board_column_card(&store, "KAN");
    let context = CommandContext { store: &store };

    Command::Card(CardCommand::RestoreSprintAttachment(
        kanban_domain::commands::card::RestoreCardSprintAttachment {
            card_id: card.id,
            sprint_id: None,
            sprint_logs: vec![],
            updated_at: chrono::Utc::now(),
        },
    ))
    .execute(&context)
    .unwrap();

    assert_all_backed(&store);
}

#[test]
fn test_set_archived_cards_sprint_leaves_its_namespace_backed() {
    let store = PrefixWriteOrderStore::new();
    let (board, _column, card) = seed_board_column_card(&store, "KAN");
    let sprint = Sprint::new(board.id, 1, None, Some("SPR"));
    store.upsert_sprint(sprint.clone()).unwrap();
    let context = CommandContext { store: &store };

    Command::Cascade(
        kanban_domain::commands::cascade_commands::CascadeCommand::SetArchivedCardsSprint(
            SetArchivedCardsSprint {
                archived_card_ids: vec![card.id],
                sprint_id: sprint.id,
            },
        ),
    )
    .execute(&context)
    .unwrap();

    assert_all_backed(&store);
}

#[test]
fn test_clear_sprint_from_archived_cards_default_leaves_its_namespace_backed() {
    let store = PrefixWriteOrderStore::new();
    let (board, _column, card) = seed_board_column_card(&store, "KAN");
    let sprint = Sprint::new(board.id, 1, None, Some("SPR"));
    store.upsert_sprint(sprint.clone()).unwrap();

    let mut updated = card.clone();
    updated.sprint_id = Some(sprint.id);
    store.upsert_card(updated).unwrap();

    let context = CommandContext { store: &store };
    Command::Card(CardCommand::Archive(ArchiveCards { ids: vec![card.id] }))
        .execute(&context)
        .unwrap();

    let dyn_store: &dyn DataStore = &store;
    dyn_store
        .clear_sprint_from_archived_cards(sprint.id, chrono::Utc::now())
        .unwrap();

    assert_all_backed(&store);
    let after = store.get_card(card.id).unwrap().unwrap();
    assert_eq!(after.sprint_id, None);
}

#[test]
fn test_the_probe_reports_a_card_written_before_its_prefix_row() {
    let store = PrefixWriteOrderStore::new();
    let mut card = Card::new(Uuid::new_v4(), Uuid::new_v4(), "C", 0);
    card.card_number = 7;
    card.prefix = "KAN".to_string();
    store.upsert_card(card).unwrap();
    assert_eq!(store.unbacked_at_write(), vec![(7, "KAN".to_string())]);

    let store2 = PrefixWriteOrderStore::new();
    store2.upsert_prefix(Prefix::new("kan")).unwrap();
    let mut card2 = Card::new(Uuid::new_v4(), Uuid::new_v4(), "C", 0);
    card2.card_number = 8;
    card2.prefix = "KAN".to_string();
    store2.upsert_card(card2).unwrap();
    assert!(store2.unbacked_at_write().is_empty());

    let store3 = PrefixWriteOrderStore::new();
    let mut card3 = Card::new(Uuid::new_v4(), Uuid::new_v4(), "C", 0);
    card3.card_number = 9;
    card3.prefix = String::new();
    store3.upsert_card(card3).unwrap();
    assert!(store3.unbacked_at_write().is_empty());
}
