mod common;

use common::prefix_write_order::PrefixWriteOrderStore;
use kanban_domain::commands::board_commands::ImportEntities;
use kanban_domain::commands::CommandContext;
use kanban_domain::{Board, Card, Column, DataStore, DomainError, KanbanError};

fn legacy_import_payload() -> ImportEntities {
    let board = Board::new("B", Some("KAN"));
    let col = Column::new(board.id, "Todo", 0);
    let mut card = Card::new(board.id, col.id, "C", 0);
    card.card_number = 7;
    ImportEntities {
        boards: vec![board],
        columns: vec![col],
        cards: vec![card],
        ..Default::default()
    }
}

#[test]
fn test_import_writes_the_prefix_row_before_the_cards_that_name_it() {
    let store = PrefixWriteOrderStore::new();
    let context = CommandContext { store: &store };
    legacy_import_payload().execute(&context).unwrap();

    let violations = store.unbacked_at_write();
    assert!(
        violations.is_empty(),
        "cards written while their namespace had no prefix row: {violations:?}"
    );

    let cards = store.list_all_cards().unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].prefix, "KAN");
    let row = store.get_prefix("kan").unwrap().unwrap();
    assert_eq!(row.card_counter, 7);
}

#[test]
fn test_import_rejects_a_card_whose_prefix_row_failed_to_land() {
    let store = PrefixWriteOrderStore::with_prefix_writes_swallowed();
    let context = CommandContext { store: &store };
    let result = legacy_import_payload().execute(&context);

    match result {
        Err(KanbanError::Domain(DomainError::PrefixNotBacked {
            card_number,
            prefix,
        })) => {
            assert_eq!(card_number, 7);
            assert_eq!(prefix, "KAN");
        }
        other => panic!("expected PrefixNotBacked, got {other:?}"),
    }

    assert!(store.list_all_cards().unwrap().is_empty());
}
