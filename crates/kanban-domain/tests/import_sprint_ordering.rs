mod common;

use common::prefix_write_order::PrefixWriteOrderStore;
use kanban_domain::commands::board_commands::ImportEntities;
use kanban_domain::commands::CommandContext;
use kanban_domain::{Board, Card, Column, DataStore, Sprint};

#[test]
fn test_import_writes_the_sprint_before_the_cards_that_reference_it() {
    let board = Board::new("B", Some("KAN"));
    let col = Column::new(board.id, "Todo", 0);
    let sprint = Sprint::new(board.id, 1, None, Some("KAN"));
    let mut card = Card::new(board.id, col.id, "C", 0);
    card.sprint_id = Some(sprint.id);

    let payload = ImportEntities {
        boards: vec![board],
        columns: vec![col],
        cards: vec![card.clone()],
        sprints: vec![sprint.clone()],
        ..Default::default()
    };

    let store = PrefixWriteOrderStore::new();
    let context = CommandContext { store: &store };
    payload.execute(&context).unwrap();

    let violations = store.unbacked_sprint_at_write();
    assert!(
        violations.is_empty(),
        "cards written while their sprint had no row: {violations:?}"
    );

    let stored = store.get_card(card.id).unwrap().unwrap();
    assert_eq!(stored.sprint_id, Some(sprint.id));
}
